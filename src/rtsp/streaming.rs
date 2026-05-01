use bytes::Bytes;
use rand::Rng;
use rtp::packet::Packet;
use rtp::packetizer::Payloader;
use rtsp_types as rtsp;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, Duration};
use webrtc::util::{Marshal, MarshalSize};

use crate::config::RtspCodec;
use crate::error::{AppError, Result};
use crate::video::encoder::registry::VideoEncoderType;
use crate::video::shared_video_pipeline::EncodedVideoFrame;
use crate::video::VideoStreamManager;
use crate::webrtc::h265_payloader::H265Payloader;

use super::bitstream::update_parameter_sets;
use super::protocol::{
    parse_rtsp_request, strip_interleaved_frames_prefix, take_rtsp_request_from_buffer,
};
use super::response::send_response;
use super::state::SharedRtspState;
use super::types::RtspRequest;

pub(crate) const RTP_CLOCK_RATE: u32 = 90_000;
pub(crate) const RTP_MTU: usize = 1200;
pub(crate) const RTSP_BUF_SIZE: usize = 8192;
const RTSP_RESUBSCRIBE_DELAY_MS: u64 = 300;

pub(crate) async fn stream_video_interleaved(
    stream: TcpStream,
    video_manager: &Arc<VideoStreamManager>,
    rtsp_codec: RtspCodec,
    channel: u8,
    shared: SharedRtspState,
    session_id: String,
) -> Result<()> {
    let (mut reader, mut writer) = stream.into_split();

    let mut rx = video_manager
        .subscribe_encoded_frames()
        .await
        .ok_or_else(|| {
            AppError::VideoError("RTSP failed to subscribe encoded frames".to_string())
        })?;

    video_manager.request_keyframe().await.ok();

    let payload_type = match rtsp_codec {
        RtspCodec::H264 => 96,
        RtspCodec::H265 => 99,
    };
    let mut sequence_number: u16 = rand::rng().random();
    let ssrc: u32 = rand::rng().random();

    let mut h264_payloader = rtp::codecs::h264::H264Payloader::default();
    let mut h265_payloader = H265Payloader::new();
    let mut ctrl_read_buf = [0u8; RTSP_BUF_SIZE];
    let mut ctrl_buffer = Vec::with_capacity(RTSP_BUF_SIZE);
    // 4-byte interleaved prefix + RTP header + payload shard (≤ RTP_MTU)
    let mut interleaved_rtp_buf = Vec::with_capacity(4 + RTP_MTU + 96);
    let mut last_rtp_timestamp: u32 = 0;

    loop {
        tokio::select! {
            maybe_frame = rx.recv() => {
                let Some(frame) = maybe_frame else {
                    tracing::warn!("RTSP encoded frame subscription ended, attempting to restart pipeline");

                    if let Some(new_rx) = video_manager.subscribe_encoded_frames().await {
                        rx = new_rx;
                        let _ = video_manager.request_keyframe().await;
                        tracing::info!("RTSP frame subscription recovered");
                    } else {
                        tracing::warn!(
                            "RTSP failed to resubscribe encoded frames, retrying in {}ms",
                            RTSP_RESUBSCRIBE_DELAY_MS
                        );
                        sleep(Duration::from_millis(RTSP_RESUBSCRIBE_DELAY_MS)).await;
                    }

                    continue;
                };

                if !is_frame_codec_match(&frame, &rtsp_codec) {
                    continue;
                }

                {
                    let mut params = shared.parameter_sets.write().await;
                    update_parameter_sets(&mut params, &frame);
                }

                let rtp_timestamp = monotonic_rtp_timestamp(
                    frame.pts_ms,
                    &mut last_rtp_timestamp,
                    frame.duration,
                );

                let payloads: Vec<Bytes> = match rtsp_codec {
                    RtspCodec::H264 => h264_payloader
                        .payload(RTP_MTU, &frame.data)
                        .map_err(|e| AppError::VideoError(format!("H264 payload failed: {}", e)))?,
                    RtspCodec::H265 => h265_payloader.payload(RTP_MTU, &frame.data),
                };

                if payloads.is_empty() {
                    continue;
                }

                let total_payloads = payloads.len();
                for (idx, payload) in payloads.into_iter().enumerate() {
                    let marker = idx == total_payloads.saturating_sub(1);
                    let packet = Packet {
                        header: rtp::header::Header {
                            version: 2,
                            padding: false,
                            extension: false,
                            marker,
                            payload_type,
                            sequence_number,
                            timestamp: rtp_timestamp,
                            ssrc,
                            ..Default::default()
                        },
                        payload,
                    };

                    sequence_number = sequence_number.wrapping_add(1);
                    send_interleaved_rtp(&mut writer, channel, &packet, &mut interleaved_rtp_buf)
                        .await?;
                }

                if frame.is_keyframe {
                    tracing::debug!("RTSP keyframe sent");
                }
            }
            read_res = reader.read(&mut ctrl_read_buf) => {
                let n = read_res?;
                if n == 0 {
                    break;
                }

                ctrl_buffer.extend_from_slice(&ctrl_read_buf[..n]);

                while strip_interleaved_frames_prefix(&mut ctrl_buffer) {}

                while let Some(raw_req) = take_rtsp_request_from_buffer(&mut ctrl_buffer) {
                    let Some(req) = parse_rtsp_request(&raw_req) else {
                        continue;
                    };

                    if handle_play_control_request(&mut writer, &req, &session_id).await? {
                        return Ok(());
                    }

                    while strip_interleaved_frames_prefix(&mut ctrl_buffer) {}
                }
            }
        }
    }

    Ok(())
}

pub(crate) async fn send_interleaved_rtp<W: AsyncWrite + Unpin>(
    stream: &mut W,
    channel: u8,
    packet: &Packet,
    marshal_buf: &mut Vec<u8>,
) -> Result<()> {
    let rtp_len = packet.marshal_size();
    let rtp_len_u16 = u16::try_from(rtp_len).map_err(|_| {
        AppError::VideoError(format!(
            "RTP packet too large for interleaved framing: {} bytes",
            rtp_len
        ))
    })?;

    marshal_buf.clear();
    marshal_buf.reserve(4 + rtp_len);
    marshal_buf.extend_from_slice(&[b'$', channel, (rtp_len_u16 >> 8) as u8, rtp_len_u16 as u8]);
    let body_off = marshal_buf.len();
    marshal_buf.resize(body_off + rtp_len, 0);

    let written = packet
        .marshal_to(&mut marshal_buf[body_off..])
        .map_err(|e| AppError::VideoError(format!("RTP marshal failed: {}", e)))?;
    if written != rtp_len {
        return Err(AppError::VideoError(format!(
            "RTP marshal size mismatch: wrote {written}, expected {rtp_len}"
        )));
    }

    stream.write_all(marshal_buf).await?;
    Ok(())
}

pub(crate) async fn handle_play_control_request<W: AsyncWrite + Unpin>(
    stream: &mut W,
    req: &RtspRequest,
    session_id: &str,
) -> Result<bool> {
    use super::protocol::OPTIONS_PUBLIC_CAPABILITIES;

    match &req.method {
        rtsp::Method::Teardown => {
            send_response(stream, req, 200, "OK", vec![], "", session_id).await?;
            Ok(true)
        }
        rtsp::Method::Options => {
            send_response(
                stream,
                req,
                200,
                "OK",
                vec![(
                    "Public".to_string(),
                    OPTIONS_PUBLIC_CAPABILITIES.to_string(),
                )],
                "",
                session_id,
            )
            .await?;
            Ok(false)
        }
        rtsp::Method::GetParameter | rtsp::Method::SetParameter => {
            send_response(stream, req, 200, "OK", vec![], "", session_id).await?;
            Ok(false)
        }
        _ => {
            send_response(
                stream,
                req,
                405,
                "Method Not Allowed",
                vec![],
                "",
                session_id,
            )
            .await?;
            Ok(false)
        }
    }
}

fn pts_to_rtp_timestamp(pts_ms: i64) -> u32 {
    if pts_ms <= 0 {
        return 0;
    }
    ((pts_ms as u64 * RTP_CLOCK_RATE as u64) / 1000) as u32
}

fn rtp_timestamp_increment(frame_duration: Duration) -> u32 {
    let inc = (frame_duration.as_secs_f64() * f64::from(RTP_CLOCK_RATE)).round() as u32;
    inc.max(1)
}

fn monotonic_rtp_timestamp(pts_ms: i64, last: &mut u32, frame_duration: Duration) -> u32 {
    let from_pts = pts_to_rtp_timestamp(pts_ms);
    let inc = rtp_timestamp_increment(frame_duration);
    let ts = if from_pts > *last {
        from_pts
    } else {
        last.wrapping_add(inc)
    };
    *last = ts;
    ts
}

fn is_frame_codec_match(frame: &EncodedVideoFrame, codec: &RtspCodec) -> bool {
    matches!(
        (frame.codec, codec),
        (VideoEncoderType::H264, RtspCodec::H264) | (VideoEncoderType::H265, RtspCodec::H265)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use tokio::io::{duplex, AsyncReadExt};

    fn make_test_request(method: rtsp::Method) -> RtspRequest {
        let mut headers = HashMap::new();
        headers.insert("cseq".to_string(), "7".to_string());
        RtspRequest {
            method,
            uri: "rtsp://127.0.0.1/live".to_string(),
            version: rtsp::Version::V1_0,
            headers,
        }
    }

    async fn read_response_from_duplex(
        mut client: tokio::io::DuplexStream,
    ) -> rtsp::Response<Vec<u8>> {
        let mut buf = vec![0u8; 4096];
        let n = client
            .read(&mut buf)
            .await
            .expect("failed to read rtsp response");
        assert!(n > 0);
        let (message, consumed): (rtsp::Message<Vec<u8>>, usize) =
            rtsp::Message::parse(&buf[..n]).expect("failed to parse rtsp response");
        assert_eq!(consumed, n);

        match message {
            rtsp::Message::Response(response) => response,
            _ => panic!("expected RTSP response"),
        }
    }

    #[tokio::test]
    async fn play_control_teardown_returns_ok_and_stop() {
        let req = make_test_request(rtsp::Method::Teardown);
        let (client, mut server) = duplex(4096);

        let should_stop = handle_play_control_request(&mut server, &req, "session-1")
            .await
            .expect("control handling failed");
        assert!(should_stop);

        drop(server);
        let response = read_response_from_duplex(client).await;
        assert_eq!(response.status(), rtsp::StatusCode::Ok);
    }

    #[tokio::test]
    async fn play_control_pause_returns_method_not_allowed() {
        let req = make_test_request(rtsp::Method::Pause);
        let (client, mut server) = duplex(4096);

        let should_stop = handle_play_control_request(&mut server, &req, "session-1")
            .await
            .expect("control handling failed");
        assert!(!should_stop);

        drop(server);
        let response = read_response_from_duplex(client).await;
        assert_eq!(response.status(), rtsp::StatusCode::MethodNotAllowed);
    }

    #[test]
    fn monotonic_rtp_timestamp_steps_when_pts_stays_zero() {
        let d = Duration::from_millis(33);
        let mut last = 0u32;
        let a = monotonic_rtp_timestamp(0, &mut last, d);
        let b = monotonic_rtp_timestamp(0, &mut last, d);
        let c = monotonic_rtp_timestamp(0, &mut last, d);
        assert!(a > 0);
        assert!(b > a);
        assert!(c > b);
    }

    #[test]
    fn monotonic_rtp_timestamp_uses_pts_when_it_advances() {
        let d = Duration::from_millis(33);
        let mut last = 0u32;
        let a = monotonic_rtp_timestamp(1000, &mut last, d);
        assert_eq!(a, 90_000);
        let b = monotonic_rtp_timestamp(2000, &mut last, d);
        assert_eq!(b, 180_000);
    }
}
