use crate::{
    common::DataFormat::{self, *},
    ffmpeg::{init_av_log, AVPixelFormat},
    ffmpeg_ram::{
        ffmpeg_linesize_offset_length, ffmpeg_ram_encode, ffmpeg_ram_free_encoder,
        ffmpeg_ram_new_encoder, ffmpeg_ram_request_keyframe, ffmpeg_ram_set_bitrate, CodecInfo, AV_NUM_DATA_POINTERS,
    },
};
use log::trace;
use std::{
    ffi::{c_void, CString},
    fmt::Display,
    os::raw::c_int,
    slice,
};

use super::Priority;
#[cfg(any(windows, target_os = "linux"))]
use crate::common::Driver;

/// Timeout for encoder test in milliseconds
const TEST_TIMEOUT_MS: u64 = 3000;

#[derive(Debug, Clone, PartialEq)]
pub struct EncodeContext {
    pub name: String,
    pub mc_name: Option<String>,
    pub width: i32,
    pub height: i32,
    pub pixfmt: AVPixelFormat,
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
                ctx.pixfmt as c_int,
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
        let mut codecs: Vec<CodecInfo> = vec![];
        #[cfg(any(windows, target_os = "linux"))]
        {
            let contains = |_vendor: Driver, _format: DataFormat| {
                // Without VRAM feature, we can't check SDK availability
                // Just return true and let FFmpeg handle the actual detection
                true
            };
            let (_nv, amf, _intel) = crate::common::supported_gpu(true);
            debug!(
                "GPU support detected - NV: {}, AMF: {}, Intel: {}",
                _nv, amf, _intel
            );

            #[cfg(windows)]
            if _intel && contains(Driver::MFX, H264) {
                codecs.push(CodecInfo {
                    name: "h264_qsv".to_owned(),
                    format: H264,
                    priority: Priority::Best as _,
                    ..Default::default()
                });
            }
            #[cfg(windows)]
            if _intel && contains(Driver::MFX, H265) {
                codecs.push(CodecInfo {
                    name: "hevc_qsv".to_owned(),
                    format: H265,
                    priority: Priority::Best as _,
                    ..Default::default()
                });
            }
            if _nv && contains(Driver::NV, H264) {
                codecs.push(CodecInfo {
                    name: "h264_nvenc".to_owned(),
                    format: H264,
                    priority: Priority::Best as _,
                    ..Default::default()
                });
            }
            if _nv && contains(Driver::NV, H265) {
                codecs.push(CodecInfo {
                    name: "hevc_nvenc".to_owned(),
                    format: H265,
                    priority: Priority::Best as _,
                    ..Default::default()
                });
            }
            if amf && contains(Driver::AMF, H264) {
                codecs.push(CodecInfo {
                    name: "h264_amf".to_owned(),
                    format: H264,
                    priority: Priority::Best as _,
                    ..Default::default()
                });
            }
            if amf {
                codecs.push(CodecInfo {
                    name: "hevc_amf".to_owned(),
                    format: H265,
                    priority: Priority::Best as _,
                    ..Default::default()
                });
            }
            #[cfg(target_os = "linux")]
            {
                codecs.push(CodecInfo {
                    name: "h264_vaapi".to_owned(),
                    format: H264,
                    priority: Priority::Good as _,
                    ..Default::default()
                });
                codecs.push(CodecInfo {
                    name: "hevc_vaapi".to_owned(),
                    format: H265,
                    priority: Priority::Good as _,
                    ..Default::default()
                });
                codecs.push(CodecInfo {
                    name: "vp8_vaapi".to_owned(),
                    format: VP8,
                    priority: Priority::Good as _,
                    ..Default::default()
                });
                codecs.push(CodecInfo {
                    name: "vp9_vaapi".to_owned(),
                    format: VP9,
                    priority: Priority::Good as _,
                    ..Default::default()
                });

                // Rockchip MPP hardware encoder support
                use std::ffi::c_int;
                extern "C" {
                    fn linux_support_rkmpp() -> c_int;
                    fn linux_support_v4l2m2m() -> c_int;
                }

                if unsafe { linux_support_rkmpp() } == 0 {
                    debug!("RKMPP hardware detected, adding Rockchip encoders");
                    codecs.push(CodecInfo {
                        name: "h264_rkmpp".to_owned(),
                        format: H264,
                        priority: Priority::Best as _,
                        ..Default::default()
                    });
                    codecs.push(CodecInfo {
                        name: "hevc_rkmpp".to_owned(),
                        format: H265,
                        priority: Priority::Best as _,
                        ..Default::default()
                    });
                }

                // V4L2 Memory-to-Memory hardware encoder support (generic ARM)
                if unsafe { linux_support_v4l2m2m() } == 0 {
                    debug!("V4L2 M2M hardware detected, adding V4L2 encoders");
                    codecs.push(CodecInfo {
                        name: "h264_v4l2m2m".to_owned(),
                        format: H264,
                        priority: Priority::Good as _,
                        ..Default::default()
                    });
                    codecs.push(CodecInfo {
                        name: "hevc_v4l2m2m".to_owned(),
                        format: H265,
                        priority: Priority::Good as _,
                        ..Default::default()
                    });
                }
            }
        }

        // qsv doesn't support yuv420p
        codecs.retain(|c| {
            let ctx = ctx.clone();
            if ctx.pixfmt == AVPixelFormat::AV_PIX_FMT_YUV420P && c.name.contains("qsv") {
                return false;
            }
            return true;
        });

        let mut res = vec![];

        if let Ok(yuv) = Encoder::dummy_yuv(ctx.clone()) {
            for codec in codecs {
                // Skip if this format already exists in results
                if res
                    .iter()
                    .any(|existing: &CodecInfo| existing.format == codec.format)
                {
                    continue;
                }

                debug!("Testing encoder: {}", codec.name);

                let c = EncodeContext {
                    name: codec.name.clone(),
                    mc_name: codec.mc_name.clone(),
                    ..ctx
                };

                match Encoder::new(c) {
                    Ok(mut encoder) => {
                        debug!("Encoder {} created successfully", codec.name);
                        let mut passed = false;
                        let mut last_err: Option<i32> = None;

                        let max_attempts = 1;
                        for attempt in 0..max_attempts {
                            let pts = (attempt as i64) * 33; // 33ms is an approximation for 30 FPS (1000 / 30)
                            let start = std::time::Instant::now();
                            match encoder.encode(&yuv, pts) {
                                Ok(frames) => {
                                    let elapsed = start.elapsed().as_millis();

                                    if frames.len() == 1 {
                                        if frames[0].key == 1 && elapsed < TEST_TIMEOUT_MS as _ {
                                            debug!(
                                                "Encoder {} test passed on attempt {}",
                                                codec.name, attempt + 1
                                            );
                                            res.push(codec.clone());
                                            passed = true;
                                            break;
                                        } else {
                                            debug!(
                                                "Encoder {} test failed on attempt {} - key: {}, timeout: {}ms",
                                                codec.name,
                                                attempt + 1,
                                                frames[0].key,
                                                elapsed
                                            );
                                        }
                                    } else {
                                        debug!(
                                            "Encoder {} test failed on attempt {} - wrong frame count: {}",
                                            codec.name,
                                            attempt + 1,
                                            frames.len()
                                        );
                                    }
                                }
                                Err(err) => {
                                    last_err = Some(err);
                                    debug!(
                                        "Encoder {} test attempt {} returned error: {}",
                                        codec.name,
                                        attempt + 1,
                                        err
                                    );
                                }
                            }
                        }

                        if !passed {
                            debug!(
                                "Encoder {} test failed after retries{}",
                                codec.name,
                                last_err
                                    .map(|e| format!(" (last err: {})", e))
                                    .unwrap_or_default()
                            );
                        }
                    }
                    Err(_) => {
                        debug!("Failed to create encoder {}", codec.name);
                    }
                }
            }
        } else {
            debug!("Failed to generate dummy YUV data");
        }

        // Add software encoders as fallback
        let soft_codecs = CodecInfo::soft();

        // Add H264 software encoder if not already present
        if !res.iter().any(|c| c.format == H264) {
            if let Some(h264_soft) = soft_codecs.h264 {
                debug!("Adding software H264 encoder: {}", h264_soft.name);
                res.push(h264_soft);
            }
        }

        // Add H265 software encoder if not already present
        if !res.iter().any(|c| c.format == H265) {
            if let Some(h265_soft) = soft_codecs.h265 {
                debug!("Adding software H265 encoder: {}", h265_soft.name);
                res.push(h265_soft);
            }
        }

        // Add VP8 software encoder if not already present
        if !res.iter().any(|c| c.format == VP8) {
            if let Some(vp8_soft) = soft_codecs.vp8 {
                debug!("Adding software VP8 encoder: {}", vp8_soft.name);
                res.push(vp8_soft);
            }
        }

        // Add VP9 software encoder if not already present
        if !res.iter().any(|c| c.format == VP9) {
            if let Some(vp9_soft) = soft_codecs.vp9 {
                debug!("Adding software VP9 encoder: {}", vp9_soft.name);
                res.push(vp9_soft);
            }
        }

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
