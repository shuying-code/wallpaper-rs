//! wallpaper-rs — cross-platform desktop wallpaper app (fixed or slideshow).

mod app;
mod autostart;
mod burnin;
mod config;
mod diskscan;
mod hotkey;
mod theme;
mod thumbs;
mod wallpaper;

use eframe::egui;

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--selftest") {
        std::process::exit(selftest());
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("wallpaper-rs {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let native = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1080.0, 720.0])
            .with_min_inner_size([900.0, 580.0])
            .with_title("Wallpaper Studio · 桌面壁纸"),
        ..Default::default()
    };
    eframe::run_native(
        "wallpaper-rs",
        native,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}

fn selftest() -> i32 {
    let mut ok = true;
    let cfg = config::Config::default();
    match serde_json::to_string(&cfg) {
        Ok(s) => match serde_json::from_str::<config::Config>(&s) {
            Ok(_) => println!("[PASS] config serialize + roundtrip"),
            Err(_) => {
                println!("[FAIL] config roundtrip");
                ok = false;
            }
        },
        Err(_) => {
            println!("[FAIL] config serialize");
            ok = false;
        }
    }
    if image::guess_format(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]).is_ok() {
        println!("[PASS] image decoder available");
    } else {
        println!("[FAIL] image decoder");
        ok = false;
    }
    // OLED burn-in pixel-shift smoke test.
    {
        let src = std::env::temp_dir().join("wp_shift_src.png");
        let dst = std::env::temp_dir().join("wp_shift_dst.png");
        let mut img = image::RgbaImage::new(8, 8);
        for (x, y, p) in img.enumerate_pixels_mut() {
            *p = image::Rgba([x as u8, y as u8, 0, 255]);
        }
        let _ = img.save(&src);
        match burnin::shifted_copy(&src, 1, 1, &dst) {
            Ok(_) => match image::open(&dst) {
                Ok(i) if i.width() == 8 && i.height() == 8 => {
                    println!("[PASS] burnin pixel shift")
                }
                _ => {
                    println!("[FAIL] burnin pixel shift (size)");
                    ok = false;
                }
            },
            Err(e) => {
                println!("[FAIL] burnin pixel shift: {e}");
                ok = false;
            }
        }
        let _ = std::fs::remove_file(&src);
        let _ = std::fs::remove_file(&dst);
    }
    println!("[INFO] config dir: {}", config::config_dir().display());
    println!("[INFO] platform: {}", std::env::consts::OS);
    let (pw, ph) = wallpaper::primary_resolution();
    println!(
        "[INFO] monitors: {} primary: {}x{}",
        wallpaper::monitor_count(),
        pw,
        ph
    );
    if ok {
        0
    } else {
        1
    }
}
