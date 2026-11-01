//! wallpaper-rs — cross-platform desktop wallpaper app (fixed or slideshow).

mod app;
mod autostart;
mod burnin;
mod config;
mod diskscan;
mod hotkey;
mod online;
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
    // Diagnostics: print / set the current desktop wallpaper (Windows).
    if args.iter().any(|a| a == "--getwp") {
        match wallpaper::current_wallpaper() {
            Some(p) => println!("{}", p.display()),
            None => println!("(unknown)"),
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--managers") {
        let m = wallpaper::conflicting_wallpaper_managers();
        if m.is_empty() {
            println!("none");
        } else {
            for x in m {
                println!("{x}");
            }
        }
        return Ok(());
    }
    if let Some(pos) = args.iter().position(|a| a == "--apply") {
        if let Some(p) = args.get(pos + 1) {
            match wallpaper::set_wallpaper(std::path::Path::new(p), wallpaper::Fit::Fill) {
                Ok(_) => println!("APPLY OK"),
                Err(e) => println!("APPLY ERR: {e}"),
            }
            println!("READBACK: {:?}", wallpaper::current_wallpaper());
            return Ok(());
        }
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
