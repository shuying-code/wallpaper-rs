//! wallpaper-rs — cross-platform desktop wallpaper app (fixed or slideshow).

mod app;
mod autostart;
mod config;
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
    println!("[INFO] config dir: {}", config::config_dir().display());
    println!("[INFO] platform: {}", std::env::consts::OS);
    if ok {
        0
    } else {
        1
    }
}
