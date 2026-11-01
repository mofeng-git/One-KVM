//! 无信号时使用的彩条画面。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// 8 段彩条（BT.601 limited range，100% 饱和度）。数值是从采集卡无信号时的
/// 原生输出逐段采样得到的，与硬件画面一致；每段等宽，整屏铺满。
const BARS: [(u8, u8, u8); 8] = [
    (235, 128, 128), // 白
    (210, 16, 145),  // 黄
    (169, 164, 16),  // 青
    (144, 54, 34),   // 绿
    (106, 200, 220), // 品红
    (81, 90, 239),   // 红
    (40, 239, 110),  // 蓝
    (16, 128, 128),  // 黑
];

/// 生成一帧 YUYV(4:2:2) 彩条。宽高需为偶数。
pub fn yuyv_bars(width: usize, height: usize) -> Vec<u8> {
    let width = width & !1;
    let height = height & !1;
    let mut buf = vec![0u8; width * height * 2];
    if width == 0 || height == 0 {
        return buf;
    }
    // 先按 x 预计算每列属于第几条
    let mut col_bar = vec![0usize; width];
    for (x, slot) in col_bar.iter_mut().enumerate() {
        *slot = (x * BARS.len() / width).min(BARS.len() - 1);
    }

    for y in 0..height {
        let row = &mut buf[y * width * 2..(y + 1) * width * 2];
        for x in (0..width).step_by(2) {
            let (y0, u, v) = BARS[col_bar[x]];
            let (y1, _, _) = BARS[col_bar[x + 1]];
            let off = x * 2;
            row[off] = y0;
            row[off + 1] = u;
            row[off + 2] = y1;
            row[off + 3] = v;
        }
    }
    buf
}

fn cache() -> &'static Mutex<HashMap<(usize, usize), Arc<Vec<u8>>>> {
    static CACHE: OnceLock<Mutex<HashMap<(usize, usize), Arc<Vec<u8>>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 带缓存的彩条帧（同尺寸只生成一次）。
pub fn cached_yuyv_bars(width: usize, height: usize) -> Arc<Vec<u8>> {
    let key = (width & !1, height & !1);
    if let Some(found) = cache().lock().ok().and_then(|m| m.get(&key).cloned()) {
        return found;
    }
    let frame = Arc::new(yuyv_bars(key.0, key.1));
    if let Ok(mut map) = cache().lock() {
        map.insert(key, frame.clone());
    }
    frame
}

/// 把彩条压成一张 JPEG（MJPEG 采集下用整帧替换）。
/// 返回空 Vec 表示生成失败，调用方应跳过替换。
pub fn jpeg_bars(width: usize, height: usize) -> Vec<u8> {
    use turbojpeg::{Compressor, Subsamp, YuvImage};

    let width = width & !1;
    let height = height & !1;
    if width == 0 || height == 0 {
        return Vec::new();
    }
    let yuyv = yuyv_bars(width, height);
    let mut i420 = vec![0u8; width * height * 3 / 2];
    if libyuv::yuy2_to_i420(&yuyv, &mut i420, width as i32, height as i32).is_err() {
        return Vec::new();
    }

    let mut compressor = match Compressor::new() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    if compressor.set_quality(85).is_err() {
        return Vec::new();
    }
    let image = YuvImage {
        pixels: i420.as_slice(),
        width,
        height,
        align: 1,
        subsamp: Subsamp::Sub2x2,
    };
    compressor.compress_yuv_to_vec(image).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// 自定义「无信号」画面：优先用用户上传的图片，没有就退回彩条
// ---------------------------------------------------------------------------

fn data_url_hash(data_url: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in data_url.as_bytes() {
        hash ^= *byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn decode_data_url_rgb(data_url: &str) -> Option<(Vec<u8>, usize, usize)> {
    use base64::Engine as _;
    let payload = data_url.split_once(',').map(|(_, rest)| rest)?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(payload)
        .ok()?;
    let image = turbojpeg::decompress(&bytes, turbojpeg::PixelFormat::RGB).ok()?;
    if image.format != turbojpeg::PixelFormat::RGB {
        return None;
    }
    Some((image.pixels, image.width, image.height))
}

/// 最近邻缩放 + RGB->YUYV（一次性开销，按图片+分辨率缓存）
fn rgb_to_yuyv_scaled(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let dw = dw & !1;
    let mut out = vec![0u8; dw * dh * 2];
    if sw == 0 || sh == 0 || dw == 0 || dh == 0 {
        return out;
    }
    for y in 0..dh {
        let sy = y * sh / dh;
        let src_row = sy * sw * 3;
        let dst_row = y * dw * 2;
        for x in (0..dw).step_by(2) {
            let sx0 = x * sw / dw;
            let sx1 = ((x + 1).min(dw - 1)) * sw / dw;
            let p0 = src_row + sx0 * 3;
            let p1 = src_row + sx1 * 3;
            let (r0, g0, b0) = (src[p0] as i32, src[p0 + 1] as i32, src[p0 + 2] as i32);
            let (r1, g1, b1) = (src[p1] as i32, src[p1 + 1] as i32, src[p1 + 2] as i32);
            let y0 = (((66 * r0 + 129 * g0 + 25 * b0 + 128) >> 8) + 16).clamp(0, 255) as u8;
            let y1 = (((66 * r1 + 129 * g1 + 25 * b1 + 128) >> 8) + 16).clamp(0, 255) as u8;
            let u = (((-38 * r0 - 74 * g0 + 112 * b0 + 128) >> 8) + 128).clamp(0, 255) as u8;
            let v = (((112 * r0 - 94 * g0 - 18 * b0 + 128) >> 8) + 128).clamp(0, 255) as u8;
            let off = dst_row + x * 2;
            out[off] = y0;
            out[off + 1] = u;
            out[off + 2] = y1;
            out[off + 3] = v;
        }
    }
    out
}

fn yuyv_to_jpeg(yuyv: &[u8], width: usize, height: usize) -> Vec<u8> {
    use turbojpeg::{Compressor, Subsamp, YuvImage};
    let width = width & !1;
    let height = height & !1;
    if width == 0 || height == 0 || yuyv.len() < width * height * 2 {
        return Vec::new();
    }
    let mut i420 = vec![0u8; width * height * 3 / 2];
    if libyuv::yuy2_to_i420(yuyv, &mut i420, width as i32, height as i32).is_err() {
        return Vec::new();
    }
    let mut compressor = match Compressor::new() {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };
    if compressor.set_quality(85).is_err() {
        return Vec::new();
    }
    let image = YuvImage {
        pixels: i420.as_slice(),
        width,
        height,
        align: 1,
        subsamp: Subsamp::Sub2x2,
    };
    compressor.compress_yuv_to_vec(image).unwrap_or_default()
}

/// 用户自定义的无信号画面（YUYV）；未配置或解码失败返回 None
pub fn custom_no_signal_yuyv(width: usize, height: usize) -> Option<Arc<Vec<u8>>> {
    let appearance = crate::appearance::current();
    let data_url = appearance.no_signal_image.as_deref()?;
    if data_url.is_empty() {
        return None;
    }
    let key = (data_url_hash(data_url), width & !1, height & !1);
    static CACHE: OnceLock<Mutex<HashMap<(u64, usize, usize), Arc<Vec<u8>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(found) = cache.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return Some(found);
    }
    let (rgb, sw, sh) = decode_data_url_rgb(data_url)?;
    let frame = Arc::new(rgb_to_yuyv_scaled(&rgb, sw, sh, key.1, key.2));
    if let Ok(mut map) = cache.lock() {
        map.clear();
        map.insert(key, frame.clone());
    }
    Some(frame)
}

/// 无信号整帧画面（YUYV）：自定义图片优先，否则彩条
pub fn cached_no_signal_yuyv(width: usize, height: usize) -> Arc<Vec<u8>> {
    custom_no_signal_yuyv(width, height).unwrap_or_else(|| cached_yuyv_bars(width, height))
}

/// 无信号整帧画面（JPEG）：自定义图片优先，否则彩条
pub fn cached_no_signal_jpeg(width: usize, height: usize) -> Arc<Vec<u8>> {
    let width = width & !1;
    let height = height & !1;
    if let Some(yuyv) = custom_no_signal_yuyv(width, height) {
        static JCACHE: OnceLock<Mutex<HashMap<(usize, usize, usize), Arc<Vec<u8>>>>> =
            OnceLock::new();
        let cache = JCACHE.get_or_init(|| Mutex::new(HashMap::new()));
        let key = (yuyv.as_ptr() as usize, width, height);
        if let Some(found) = cache.lock().ok().and_then(|m| m.get(&key).cloned()) {
            return found;
        }
        let jpeg = yuyv_to_jpeg(&yuyv, width, height);
        if !jpeg.is_empty() {
            let frame = Arc::new(jpeg);
            if let Ok(mut map) = cache.lock() {
                map.clear();
                map.insert(key, frame.clone());
            }
            return frame;
        }
    }
    cached_jpeg_bars(width, height)
}

/// 带缓存的 JPEG 彩条帧（同尺寸只生成一次）。
pub fn cached_jpeg_bars(width: usize, height: usize) -> Arc<Vec<u8>> {
    static CACHE: OnceLock<Mutex<HashMap<(usize, usize), Arc<Vec<u8>>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (width & !1, height & !1);
    if let Some(found) = cache.lock().ok().and_then(|m| m.get(&key).cloned()) {
        return found;
    }
    let frame = Arc::new(jpeg_bars(key.0, key.1));
    if let Ok(mut map) = cache.lock() {
        map.insert(key, frame.clone());
    }
    frame
}
