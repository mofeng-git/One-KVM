use crate::{
    common::DataFormat::{self, *},
    ffmpeg::{init_av_log, AVPixelFormat},
    ffmpeg_ram::{
        ffmpeg_linesize_offset_length, ffmpeg_ram_encode, ffmpeg_ram_free_encoder,
        ffmpeg_ram_new_encoder, ffmpeg_ram_request_keyframe, ffmpeg_ram_set_bitrate, CodecInfo,
        AV_NUM_DATA_POINTERS,
    },
};
use log::trace;
use std::{
    ffi::{c_void, CString},
    fmt::Display,
    os::raw::c_int,
    slice,
};

#[cfg(any(windows, target_os = "linux"))]
use crate::common::Driver;

/// Timeout for encoder test in milliseconds
const TEST_TIMEOUT_MS: u64 = 3000;
const PRIORITY_NVENC: i32 = 0;
const PRIORITY_QSV: i32 = 1;
const PRIORITY_AMF: i32 = 2;
const PRIORITY_RKMPP: i32 = 3;
const PRIORITY_VAAPI: i32 = 4;
const PRIORITY_V4L2M2M: i32 = 5;

#[derive(Clone, Copy)]
struct CandidateCodecSpec {
    name: &'static str,
    format: DataFormat,
    priority: i32,
}

fn push_candidate(codecs: &mut Vec<CodecInfo>, candidate: CandidateCodecSpec) {
    codecs.push(CodecInfo {
        name: candidate.name.to_owned(),
        format: candidate.format,
        priority: candidate.priority,
        ..Default::default()
    });
}

#[cfg(target_os = "linux")]
fn linux_support_vaapi() -> bool {
    let entries = match std::fs::read_dir("/dev/dri") {
        Ok(entries) => entries,
        Err(_) => return false,
    };

    entries.flatten().any(|entry| {
        entry
            .file_name()
            .to_str()
            .map(|name| name.starts_with("renderD"))
            .unwrap_or(false)
    })
}

#[cfg(not(target_os = "linux"))]
fn linux_support_vaapi() -> bool {
    false
}

#[cfg(target_os = "linux")]
fn linux_support_rkmpp() -> bool {
    extern "C" {
        fn linux_support_rkmpp() -> c_int;
    }

    unsafe { linux_support_rkmpp() == 0 }
}

#[cfg(not(target_os = "linux"))]
fn linux_support_rkmpp() -> bool {
    false
}

#[cfg(target_os = "linux")]
fn linux_support_v4l2m2m() -> bool {
    extern "C" {
        fn linux_support_v4l2m2m() -> c_int;
    }

    unsafe { linux_support_v4l2m2m() == 0 }
}

#[cfg(not(target_os = "linux"))]
fn linux_support_v4l2m2m() -> bool {
    false
}

#[cfg(any(windows, target_os = "linux"))]
fn enumerate_candidate_codecs(ctx: &EncodeContext) -> Vec<CodecInfo> {
    use log::debug;

    let mut codecs = Vec::new();
    let contains = |_vendor: Driver, _format: DataFormat| {
        // Without VRAM feature, we can't check SDK availability.
        // Keep the prefilter coarse and let FFmpeg validation do the real check.
        true
    };
    let (nv, amf, intel) = crate::common::supported_gpu(true);

    debug!(
        "GPU support detected - NV: {}, AMF: {}, Intel: {}",
        nv, amf, intel
    );

    if nv && contains(Driver::NV, H264) {
        push_candidate(
            &mut codecs,
            CandidateCodecSpec {
                name: "h264_nvenc",
                format: H264,
                priority: PRIORITY_NVENC,
            },
        );
    }
    if nv && contains(Driver::NV, H265) {
        push_candidate(
            &mut codecs,
            CandidateCodecSpec {
                name: "hevc_nvenc",
                format: H265,
                priority: PRIORITY_NVENC,
            },
        );
    }
    if intel && contains(Driver::MFX, H264) {
        push_candidate(
            &mut codecs,
            CandidateCodecSpec {
                name: "h264_qsv",
                format: H264,
                priority: PRIORITY_QSV,
            },
        );
    }
    if intel && contains(Driver::MFX, H265) {
        push_candidate(
            &mut codecs,
            CandidateCodecSpec {
                name: "hevc_qsv",
                format: H265,
                priority: PRIORITY_QSV,
            },
        );
    }
    if amf && contains(Driver::AMF, H264) {
        push_candidate(
            &mut codecs,
            CandidateCodecSpec {
                name: "h264_amf",
                format: H264,
                priority: PRIORITY_AMF,
            },
        );
    }
    if amf && contains(Driver::AMF, H265) {
        push_candidate(
            &mut codecs,
            CandidateCodecSpec {
                name: "hevc_amf",
                format: H265,
                priority: PRIORITY_AMF,
            },
        );
    }
    if linux_support_rkmpp() {
        debug!("RKMPP hardware detected, adding Rockchip encoders");
        push_candidate(
            &mut codecs,
            CandidateCodecSpec {
                name: "h264_rkmpp",
                format: H264,
                priority: PRIORITY_RKMPP,
            },
        );
        push_candidate(
            &mut codecs,
            CandidateCodecSpec {
                name: "hevc_rkmpp",
                format: H265,
                priority: PRIORITY_RKMPP,
            },
        );
    }
    if cfg!(target_os = "linux") && linux_support_vaapi() {
        push_candidate(
            &mut codecs,
            CandidateCodecSpec {
                name: "h264_vaapi",
                format: H264,
                priority: PRIORITY_VAAPI,
            },
        );
        push_candidate(
            &mut codecs,
            CandidateCodecSpec {
                name: "hevc_vaapi",
                format: H265,
                priority: PRIORITY_VAAPI,
            },
        );
        push_candidate(
            &mut codecs,
            CandidateCodecSpec {
                name: "vp8_vaapi",
                format: VP8,
                priority: PRIORITY_VAAPI,
            },
        );
        push_candidate(
            &mut codecs,
            CandidateCodecSpec {
                name: "vp9_vaapi",
                format: VP9,
                priority: PRIORITY_VAAPI,
            },
        );
    }
    if linux_support_v4l2m2m() {
        debug!("V4L2 M2M hardware detected, adding V4L2 encoders");
        push_candidate(
            &mut codecs,
            CandidateCodecSpec {
                name: "h264_v4l2m2m",
                format: H264,
                priority: PRIORITY_V4L2M2M,
            },
        );
        push_candidate(
            &mut codecs,
            CandidateCodecSpec {
                name: "hevc_v4l2m2m",
                format: H265,
                priority: PRIORITY_V4L2M2M,
            },
        );
    }

    codecs.retain(|codec| {
        !(ctx.pixfmt == AVPixelFormat::AV_PIX_FMT_YUV420P as i32
            && codec.name.contains("qsv"))
    });
    codecs
}

#[derive(Clone, Copy)]
struct ProbePolicy {
    max_attempts: usize,
    request_keyframe: bool,
    accept_any_output: bool,
}

impl ProbePolicy {
    fn for_codec(codec_name: &str) -> Self {
        if codec_name.contains("v4l2m2m") {
            Self {
                max_attempts: 5,
                request_keyframe: true,
                accept_any_output: true,
            }
        } else {
            Self {
                max_attempts: 1,
                request_keyframe: false,
                accept_any_output: false,
            }
        }
    }

    fn prepare_attempt(&self, encoder: &mut Encoder) {
        if self.request_keyframe {
            encoder.request_keyframe();
        }
    }

    fn passed(&self, frames: &[EncodeFrame], elapsed_ms: u128) -> bool {
        if elapsed_ms >= TEST_TIMEOUT_MS as u128 {
            return false;
        }

        if self.accept_any_output {
            !frames.is_empty()
        } else {
            frames.len() == 1 && frames[0].key == 1
        }
    }
}

fn log_failed_probe_attempt(
    codec_name: &str,
    policy: ProbePolicy,
    attempt: usize,
    frames: &[EncodeFrame],
    elapsed_ms: u128,
) {
    use log::debug;

    if policy.accept_any_output {
        if frames.is_empty() {
            debug!(
                "Encoder {} test produced no output on attempt {}",
                codec_name, attempt
            );
        } else {
            debug!(
                "Encoder {} test failed on attempt {} - frames: {}, timeout: {}ms",
                codec_name,
                attempt,
                frames.len(),
                elapsed_ms
            );
        }
    } else if frames.len() == 1 {
        debug!(
            "Encoder {} test failed on attempt {} - key: {}, timeout: {}ms",
            codec_name, attempt, frames[0].key, elapsed_ms
        );
    } else {
        debug!(
            "Encoder {} test failed on attempt {} - wrong frame count: {}",
            codec_name,
            attempt,
            frames.len()
        );
    }
}

fn validate_candidate(codec: &CodecInfo, ctx: &EncodeContext, yuv: &[u8]) -> bool {
    use log::debug;

    debug!("Testing encoder: {}", codec.name);

    let test_ctx = EncodeContext {
        name: codec.name.clone(),
        mc_name: codec.mc_name.clone(),
        ..ctx.clone()
    };

    match Encoder::new(test_ctx) {
        Ok(mut encoder) => {
            debug!("Encoder {} created successfully", codec.name);
            let policy = ProbePolicy::for_codec(&codec.name);
            let mut last_err: Option<i32> = None;

            for attempt in 0..policy.max_attempts {
                let attempt_no = attempt + 1;
                policy.prepare_attempt(&mut encoder);

                let pts = (attempt as i64) * 33;
                let start = std::time::Instant::now();
                match encoder.encode(yuv, pts) {
                    Ok(frames) => {
                        let elapsed = start.elapsed().as_millis();

                        if policy.passed(frames, elapsed) {
                            if policy.accept_any_output {
                                debug!(
                                    "Encoder {} test passed on attempt {} (frames: {})",
                                    codec.name,
                                    attempt_no,
                                    frames.len()
                                );
                            } else {
                                debug!(
                                    "Encoder {} test passed on attempt {}",
                                    codec.name, attempt_no
                                );
                            }
                            return true;
                        } else {
                            log_failed_probe_attempt(
                                &codec.name,
                                policy,
                                attempt_no,
                                frames,
                                elapsed,
                            );
                        }
                    }
                    Err(err) => {
                        last_err = Some(err);
                        debug!(
                            "Encoder {} test attempt {} returned error: {}",
                            codec.name, attempt_no, err
                        );
                    }
                }
            }

            debug!(
                "Encoder {} test failed after retries{}",
                codec.name,
                last_err
                    .map(|e| format!(" (last err: {})", e))
                    .unwrap_or_default()
            );
            false
        }
        Err(_) => {
            debug!("Failed to create encoder {}", codec.name);
            false
        }
    }
}

fn add_software_fallback(codecs: &mut Vec<CodecInfo>) {
    use log::debug;

    for fallback in CodecInfo::soft().into_vec() {
        if !codecs.iter().any(|codec| codec.format == fallback.format) {
            debug!(
                "Adding software {:?} encoder: {}",
                fallback.format, fallback.name
            );
            codecs.push(fallback);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EncodeContext {
    pub name: String,
    pub mc_name: Option<String>,
    pub width: i32,
    pub height: i32,
    pub pixfmt: i32,
    pub align: i32,
    pub fps: i32,
    pub gop: i32,
    pub rc: crate::common::RateControl,
    pub quality: crate::common::Quality,
    pub kbs: i32,
    pub q: i32,
    pub thread_count: i32,
}

pub struct EncodeFrame {
    pub data: Vec<u8>,
    pub pts: i64,
    pub key: i32,
}

impl Display for EncodeFrame {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "encode len:{}, pts:{}", self.data.len(), self.pts)
    }
}

pub struct Encoder {
    codec: *mut c_void,
    frames: *mut Vec<EncodeFrame>,
    pub ctx: EncodeContext,
    pub linesize: Vec<i32>,
    pub offset: Vec<i32>,
    pub length: i32,
}

impl Encoder {
    pub fn new(ctx: EncodeContext) -> Result<Self, ()> {
        init_av_log();
        if ctx.width % 2 == 1 || ctx.height % 2 == 1 {
            return Err(());
        }
        unsafe {
            let mut linesize = Vec::<i32>::new();
            linesize.resize(AV_NUM_DATA_POINTERS as _, 0);
            let mut offset = Vec::<i32>::new();
            offset.resize(AV_NUM_DATA_POINTERS as _, 0);
            let mut length = Vec::<i32>::new();
            length.resize(1, 0);
            let gpu = std::env::var("RUSTDESK_HWCODEC_NVENC_GPU")
                .unwrap_or("-1".to_owned())
                .parse()
                .unwrap_or(-1);
            let mc_name = ctx.mc_name.clone().unwrap_or_default();
            let codec = ffmpeg_ram_new_encoder(
                CString::new(ctx.name.as_str()).map_err(|_| ())?.as_ptr(),
                CString::new(mc_name.as_str()).map_err(|_| ())?.as_ptr(),
                ctx.width,
                ctx.height,
                ctx.pixfmt,
                ctx.align,
                ctx.fps,
                ctx.gop,
                ctx.rc as _,
                ctx.quality as _,
                ctx.kbs,
                ctx.q,
                ctx.thread_count,
                gpu,
                linesize.as_mut_ptr(),
                offset.as_mut_ptr(),
                length.as_mut_ptr(),
                Some(Encoder::callback),
            );

            if codec.is_null() {
                return Err(());
            }

            Ok(Encoder {
                codec,
                frames: Box::into_raw(Box::new(Vec::<EncodeFrame>::new())),
                ctx,
                linesize,
                offset,
                length: length[0],
            })
        }
    }

    pub fn encode(&mut self, data: &[u8], ms: i64) -> Result<&mut Vec<EncodeFrame>, i32> {
        unsafe {
            (&mut *self.frames).clear();
            let result = ffmpeg_ram_encode(
                self.codec,
                (*data).as_ptr(),
                data.len() as _,
                self.frames as *const _ as *const c_void,
                ms,
            );
            // ffmpeg_ram_encode returns AVERROR(EAGAIN) when the encoder accepts the frame
            // but does not output a packet yet (e.g., startup delay / internal buffering).
            // Treat this as a successful call with an empty output list.
            if result == -11 {
                return Ok(&mut *self.frames);
            }
            if result != 0 {
                return Err(result);
            }
            Ok(&mut *self.frames)
        }
    }

    extern "C" fn callback(data: *const u8, size: c_int, pts: i64, key: i32, obj: *const c_void) {
        unsafe {
            let frames = &mut *(obj as *mut Vec<EncodeFrame>);
            frames.push(EncodeFrame {
                data: slice::from_raw_parts(data, size as _).to_vec(),
                pts,
                key,
            });
        }
    }

    pub fn set_bitrate(&mut self, kbs: i32) -> Result<(), ()> {
        let ret = unsafe { ffmpeg_ram_set_bitrate(self.codec, kbs) };
        if ret == 0 {
            Ok(())
        } else {
            Err(())
        }
    }

    /// Request next frame to be encoded as a keyframe (IDR)
    pub fn request_keyframe(&mut self) {
        unsafe {
            ffmpeg_ram_request_keyframe(self.codec);
        }
    }

    pub fn format_from_name(name: String) -> Result<DataFormat, ()> {
        if name.contains("h264") {
            return Ok(H264);
        } else if name.contains("hevc") {
            return Ok(H265);
        } else if name.contains("vp8") {
            return Ok(VP8);
        } else if name.contains("vp9") {
            return Ok(VP9);
        } else if name.contains("av1") {
            return Ok(AV1);
        }
        Err(())
    }

    pub fn available_encoders(ctx: EncodeContext, _sdk: Option<String>) -> Vec<CodecInfo> {
        use log::debug;

        if !(cfg!(windows) || cfg!(target_os = "linux")) {
            return vec![];
        }
        let mut res = vec![];
        #[cfg(any(windows, target_os = "linux"))]
        let codecs = enumerate_candidate_codecs(&ctx);

        if let Ok(yuv) = Encoder::dummy_yuv(ctx.clone()) {
            for codec in codecs {
                if validate_candidate(&codec, &ctx, &yuv) {
                    res.push(codec);
                }
            }
        } else {
            debug!("Failed to generate dummy YUV data");
        }

        add_software_fallback(&mut res);

        res
    }

    fn dummy_yuv(ctx: EncodeContext) -> Result<Vec<u8>, ()> {
        let mut yuv = vec![];
        if let Ok((_, _, len)) = ffmpeg_linesize_offset_length(
            ctx.pixfmt,
            ctx.width as _,
            ctx.height as _,
            ctx.align as _,
        ) {
            yuv.resize(len as _, 0);
            return Ok(yuv);
        }

        Err(())
    }
}

impl Drop for Encoder {
    fn drop(&mut self) {
        unsafe {
            ffmpeg_ram_free_encoder(self.codec);
            self.codec = std::ptr::null_mut();
            let _ = Box::from_raw(self.frames);
            trace!("Encoder dropped");
        }
    }
}
