//! OLED burn-in guard: produce a slightly shifted copy of the current wallpaper
//! and apply it, so static UI elements on screen (taskbar, clock, icons) map to
//! different physical pixels over time and are less likely to leave a ghost.

use crate::wallpaper::{self, Fit};
use std::path::{Path, PathBuf};

/// Create a copy of `path` shifted by (dx, dy) pixels (edge-clamped) and save it
/// to `out`. The whole image stays the same size so the desktop layout is kept.
pub fn shifted_copy(path: &Path, dx: i64, dy: i64, out: &Path) -> Result<(), String> {
    let img = image::open(path).map_err(|e| e.to_string())?;
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return Err("图片尺寸无效".to_string());
    }
    let rgba = img.to_rgba8();
    let src = rgba.as_raw();
    let mut dst = vec![0u8; (w as usize) * (h as usize) * 4];
    let wi = w as i64;
    let hi = h as i64;
    for y in 0..h as i64 {
        let sy = (y - dy).clamp(0, hi - 1) as usize;
        let row_src = (sy as usize) * (w as usize) * 4;
        let row_dst = (y as usize) * (w as usize) * 4;
        for x in 0..w as i64 {
            let sx = (x - dx).clamp(0, wi - 1) as usize;
            let si = row_src + sx * 4;
            let di = row_dst + (x as usize) * 4;
            dst[di..di + 4].copy_from_slice(&src[si..si + 4]);
        }
    }
    let out_img = image::RgbaImage::from_raw(w, h, dst).ok_or("构建图像失败")?;
    out_img.save(out).map_err(|e| e.to_string())
}

/// Shift the wallpaper by (dx, dy) and apply the shifted copy to the desktop.
/// Temporary file lives in `out_dir` (the app config directory).
pub fn apply_shift(path: &Path, fit: Fit, dx: i64, dy: i64, out_dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out_dir).ok();
    let out: PathBuf = out_dir.join("oled_shift.png");
    shifted_copy(path, dx, dy, &out)?;
    wallpaper::set_wallpaper(&out, fit)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_src(dir: &Path) -> PathBuf {
        // 4x4 image; R = x*10+y encodes the coordinate so we can verify mapping.
        let mut img = image::RgbaImage::new(4, 4);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgba([(x * 10 + y) as u8, 0, 0, 255]);
        }
        let p = dir.join("src.png");
        img.save(&p).unwrap();
        p
    }

    #[test]
    fn shift_maps_pixels_and_clamps_edges() {
        let dir = std::env::temp_dir().join("wp_burnin_test");
        std::fs::create_dir_all(&dir).unwrap();
        let src = make_src(&dir);
        let dst = dir.join("dst.png");

        // Shift content right by 1 (dx=+1): out(x) = src(x-1), clamped at left.
        shifted_copy(&src, 1, 0, &dst).unwrap();
        let out = image::open(&dst).unwrap().to_rgba8();
        assert_eq!(out.dimensions(), (4, 4));
        let red = |x: u32, y: u32| out.get_pixel(x, y)[0];
        // x=0 clamps to src(0); x=1..3 copy src(x-1).
        assert_eq!(red(0, 0), 0); // src(0,0)=0
        assert_eq!(red(1, 0), 0); // src(0,0)=0
        assert_eq!(red(2, 0), 10); // src(1,0)=10
        assert_eq!(red(3, 0), 20); // src(2,0)=20
    }

    #[test]
    fn shift_down_maps_rows() {
        let dir = std::env::temp_dir().join("wp_burnin_test2");
        std::fs::create_dir_all(&dir).unwrap();
        let src = make_src(&dir);
        let dst = dir.join("dst.png");

        // Shift content down by 1 (dy=+1): out(y) = src(y-1), clamped at top.
        shifted_copy(&src, 0, 1, &dst).unwrap();
        let out = image::open(&dst).unwrap().to_rgba8();
        let red = |x: u32, y: u32| out.get_pixel(x, y)[0];
        // Column x=0: R = y so out(0,0)=0 (clamp), out(0,1)=0, out(0,2)=1...
        assert_eq!(red(0, 0), 0);
        assert_eq!(red(0, 1), 0);
        assert_eq!(red(0, 2), 1);
        assert_eq!(red(0, 3), 2);
    }
}
