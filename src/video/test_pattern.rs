//! 无信号时使用的彩条画面。

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

/// 75% 彩条（BT.601 limited range）的 YUV 值
const BARS: [(u8, u8, u8); 7] = [
    (180, 128, 128), // 白
    (161, 44, 142),  // 黄
    (131, 156, 44),  // 青
    (112, 72, 58),   // 绿
    (84, 184, 198),  // 品红
    (65, 100, 212),  // 红
    (35, 212, 114),  // 蓝
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
        *slot = (x * 7 / width).min(6);
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
