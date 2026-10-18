//! Pull the latest JPEG directly when the HTTP consumer is ready.

use std::io;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures::Stream;
use tokio::sync::broadcast::error::{RecvError, TryRecvError};

use super::{ClientGuard, MjpegStreamHandler};

const PART_HEADER: &[u8] = b"--frame\r\nContent-Type: image/jpeg\r\n\r\n";
// Chromium commits the previous image after parsing the next part's complete
// headers. Send these immediately after the JPEG, even when the scene stops.
const PART_TRAILER: &[u8] = b"\r\n--frame\r\nContent-Type: image/jpeg\r\n\r\n";

pub(crate) fn body_stream(
    handler: Arc<MjpegStreamHandler>,
    guard: ClientGuard,
) -> impl Stream<Item = Result<Bytes, io::Error>> + Send {
    stream_with_timeout(handler, guard, Duration::from_secs(5))
}

fn stream_with_timeout(
    handler: Arc<MjpegStreamHandler>,
    guard: ClientGuard,
    timeout: Duration,
) -> impl Stream<Item = Result<Bytes, io::Error>> + Send {
    // Subscribe before reading the initial frame so an update cannot be missed.
    let mut notifications = handler.subscribe();
    async_stream::stream! {
        let guard = guard;
        let mut last_sequence = None;
        let mut timeout_count = 0;
        let mut keepalive = false;

        yield Ok(Bytes::from_static(PART_HEADER));
        loop {
            // Notifications contain no frame data. Coalesce the backlog, then
            // fetch the latest frame rather than queueing JPEGs for slow peers.
            loop {
                match notifications.try_recv() {
                    Ok(()) | Err(TryRecvError::Lagged(_)) => timeout_count = 0,
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Closed) => return,
                }
            }
            if !handler.is_online() {
                break;
            }

            if let Some(frame) = handler.current_frame() {
                if (keepalive || last_sequence != Some(frame.sequence)) && frame.is_valid_jpeg() {
                    last_sequence = Some(frame.sequence);
                    // This retains the pooled owner; it does not copy the JPEG.
                    let data = frame.owned_data_bytes();
                    drop(frame);
                    handler.record_frame_sent(guard.id());
                    yield Ok(data);
                    yield Ok(Bytes::from_static(PART_TRAILER));
                }
            }

            keepalive = false;
            match tokio::time::timeout(timeout, notifications.recv()).await {
                Ok(Ok(())) | Ok(Err(RecvError::Lagged(_))) => timeout_count = 0,
                Ok(Err(RecvError::Closed)) => break,
                Err(_) => {
                    timeout_count += 1;
                    if timeout_count > 6 {
                        break;
                    }
                    keepalive = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::video::format::{PixelFormat, Resolution};
    use crate::video::frame::{FrameBuffer, FrameBufferPool, VideoFrame};
    use futures::StreamExt;

    fn jpeg(sequence: u64) -> VideoFrame {
        let mut data = vec![sequence as u8; 128];
        data[..2].copy_from_slice(&[0xff, 0xd8]);
        data[126..].copy_from_slice(&[0xff, 0xd9]);
        VideoFrame::new(
            Bytes::from(data),
            Resolution::VGA,
            PixelFormat::Mjpeg,
            0,
            sequence,
        )
    }

    #[tokio::test]
    async fn sends_complete_pooled_frame_without_copying_or_waiting_for_next_frame() {
        let handler = Arc::new(MjpegStreamHandler::with_drop_limit(0));
        let pool = Arc::new(FrameBufferPool::new(1));
        let data = jpeg(0).data().to_vec();
        let original = data.as_ptr();
        handler.update_frame(VideoFrame::from_pooled(
            Arc::new(FrameBuffer::new(data, Some(pool.clone()))),
            Resolution::VGA,
            PixelFormat::Mjpeg,
            0,
            0,
        ));
        let guard = ClientGuard::new("http-test".into(), handler.clone());
        let mut stream = Box::pin(body_stream(handler.clone(), guard));
        assert_eq!(stream.next().await.unwrap().unwrap().as_ref(), PART_HEADER);
        let payload = stream.next().await.unwrap().unwrap();
        assert_eq!(payload.as_ptr(), original);
        assert_eq!(stream.next().await.unwrap().unwrap().as_ref(), PART_TRAILER);
        assert!(
            tokio::time::timeout(Duration::from_millis(20), stream.next())
                .await
                .is_err()
        );

        // Exceed the broadcast capacity while the HTTP consumer is stalled.
        for sequence in 1..=40 {
            handler.update_frame(jpeg(sequence));
        }
        let latest = stream.next().await.unwrap().unwrap();
        assert_eq!(latest[2], 40);
        assert_eq!(stream.next().await.unwrap().unwrap().as_ref(), PART_TRAILER);
        // The previous pooled frame remains valid until its HTTP owner drops.
        assert_eq!(payload.as_ptr(), original);
        assert_eq!(payload[2], 0);
        drop(payload);
        let recycled = pool.take(128);
        assert_eq!(recycled.as_ptr(), original);

        // Capture sequence numbers may reset after reopening the source.
        handler.update_frame(jpeg(0));
        assert_eq!(stream.next().await.unwrap().unwrap()[2], 0);
        assert_eq!(stream.next().await.unwrap().unwrap().as_ref(), PART_TRAILER);
        assert_eq!(handler.get_clients_stat()["http-test"].fps, 3);
        drop(stream);
        assert_eq!(handler.client_count(), 0);
    }

    #[tokio::test]
    async fn keepalive_and_offline_close_release_the_client() {
        let handler = Arc::new(MjpegStreamHandler::new());
        handler.update_frame(jpeg(1));
        let guard = ClientGuard::new("keepalive-test".into(), handler.clone());
        let mut stream = Box::pin(stream_with_timeout(
            handler.clone(),
            guard,
            Duration::from_millis(10),
        ));
        for _ in 0..3 {
            stream.next().await.unwrap().unwrap();
        }
        let repeated = tokio::time::timeout(Duration::from_secs(1), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(repeated[2], 1);
        assert_eq!(stream.next().await.unwrap().unwrap().as_ref(), PART_TRAILER);
        handler.set_offline();
        assert!(stream.next().await.is_none());
        drop(stream);
        assert_eq!(handler.client_count(), 0);
    }

    #[test]
    fn dropping_an_unpolled_response_releases_the_client() {
        let handler = Arc::new(MjpegStreamHandler::new());
        let guard = ClientGuard::new("unpolled-test".into(), handler.clone());
        let stream = body_stream(handler.clone(), guard);
        assert_eq!(handler.client_count(), 1);
        drop(stream);
        assert_eq!(handler.client_count(), 0);
    }
}
