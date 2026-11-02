//! wallpaper-rs — cross-platform desktop wallpaper app (fixed or slideshow).
//!
//! Thin desktop entry point; all UI/logic lives in the `wallpaper_rs` library so
//! the same code can also be packaged as an Android APK.

// Windows release builds are GUI apps: don't spawn a black console window.
// Debug builds keep the console for `--selftest` / `--shots` / diagnostics.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]

use wallpaper_rs::{app, config, wallpaper};

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
    // Screen-size adaptation screenshots: `--shots <dir>`.
    if let Some(pos) = args.iter().position(|a| a == "--shots") {
        if let Some(dir) = args.get(pos + 1) {
            std::env::set_var("WP_SHOT_DIR", dir);
        } else {
            eprintln!("--shots requires an output directory");
            std::process::exit(2);
        }
    }

    let native = wallpaper_rs::native_options();
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
