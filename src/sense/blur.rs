//! 离开时虚化壁纸：真的把当前桌面壁纸换成一张模糊版本。
//!
//! 实现是「降采样 → 放大」的方框模糊近似，纯 CPU、无新依赖；结果写进配置目录，
//! 离开时设为壁纸，人回来再还原原图。因为要读写图片，全部在后台线程执行。

use crate::wallpaper::Fit;
use image::{imageops, RgbaImage};
use std::path::{Path, PathBuf};

fn blurred_path() -> PathBuf {
    crate::config::config_dir().join("sense-blur.png")
}

/// 生成模糊图（缩到 1/N 再插值放大，等价于大半径方框模糊）。
pub fn make_blurred(src: &Path, out: &Path, factor: u32) -> Result<(), String> {
    let img = image::open(src).map_err(|e| format!("读取壁纸失败：{e}"))?;
    let rgba: RgbaImage = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    if w < 4 || h < 4 {
        return Err("壁纸尺寸过小，跳过虚化".to_string());
    }
    let f = factor.clamp(2, 64);
    let (sw, sh) = ((w / f).max(1), (h / f).max(1));
    let small = imageops::resize(&rgba, sw, sh, imageops::FilterType::Triangle);
    // 再模糊一次，避免放大后的块状边缘。
    let small = imageops::blur(&small, (f as f32 / 3.0).max(1.0));
    let big = imageops::resize(&small, w, h, imageops::FilterType::Triangle);
    if let Some(dir) = out.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    big.save(out).map_err(|e| format!("写入模糊图失败：{e}"))?;
    Ok(())
}

/// 后台虚化：生成 + 设为壁纸。返回 `Ok(原壁纸路径)`，用于还原。
pub fn apply_async(src: PathBuf) -> std::sync::mpsc::Receiver<Result<PathBuf, String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let out = blurred_path();
        let res = make_blurred(&src, &out, 14).and_then(|()| {
            crate::wallpaper::set_wallpaper(&out, Fit::Fill)?;
            Ok(src.clone())
        });
        let _ = tx.send(res);
    });
    rx
}

/// 还原原壁纸（同样在后台）。
pub fn restore_async(src: PathBuf) -> std::sync::mpsc::Receiver<Result<(), String>> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let res = if src.is_file() {
            crate::wallpaper::set_wallpaper(&src, Fit::Fill)
        } else {
            Err(format!("原壁纸已不存在：{}", src.display()))
        };
        let _ = tx.send(res);
    });
    rx
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blurs_a_synthetic_image() {
        let dir = std::env::temp_dir().join(format!("wp_blur_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("in.png");
        let out = dir.join("out.png");
        let mut img = RgbaImage::new(64, 64);
        for (x, y, px) in img.enumerate_pixels_mut() {
            *px = if (x / 8 + y / 8) % 2 == 0 {
                image::Rgba([255, 255, 255, 255])
            } else {
                image::Rgba([0, 0, 0, 255])
            };
        }
        img.save(&src).unwrap();
        make_blurred(&src, &out, 6).expect("blur");
        let a = image::open(&src).unwrap().to_rgba8();
        let b = image::open(&out).unwrap().to_rgba8();
        assert_eq!(a.dimensions(), b.dimensions(), "尺寸必须保持");
        // 模糊后相邻像素差应显著变小（原来的硬边被抹平）。
        let contrast = |im: &RgbaImage| {
            let (w, h) = im.dimensions();
            let mut sum = 0i64;
            for y in 0..h {
                for x in 1..w {
                    let p = im.get_pixel(x, y)[0] as i64;
                    let q = im.get_pixel(x - 1, y)[0] as i64;
                    sum += (p - q).abs();
                }
            }
            sum
        };
        assert!(contrast(&b) < contrast(&a), "模糊后对比度必须下降");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
