//! wallpaper-rs — cross-platform desktop wallpaper app (fixed or slideshow).
//!
//! Thin desktop entry point; all UI/logic lives in the `wallpaper_rs` library so
//! the same code can also be packaged as an Android APK.

// Windows release builds are GUI apps: don't spawn a black console window.
// Debug builds keep the console for `--selftest` / `--shots` / diagnostics.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")] // 语句

use wallpaper_rs::{app, config, wallpaper}; // 引入依赖

fn main() -> eframe::Result<()> { // 函数 main
    let args: Vec<String> = std::env::args().collect(); // 变量 args
    // Elevated helper: this copy was launched through the UAC prompt by
    // `elevate::run_elevated`. Do the privileged work, exit with a status code,
    // and never open a window.
    if let Some((action, rest)) = wallpaper_rs::elevate::pending_helper_call(&args) { // 条件分支
        std::process::exit(wallpaper_rs::elevate::run_helper_action(&action, &rest)); // 调用 exit
    } // 块结束
    if args.iter().any(|a| a == "--selftest") { // 条件分支
        std::process::exit(selftest()); // 调用 exit
    } // 块结束
    if args.iter().any(|a| a == "--version" || a == "-V") { // 条件分支
        println!("wallpaper-rs {}", env!("CARGO_PKG_VERSION")); // 语句
        return Ok(()); // 返回
    } // 块结束
    // Diagnostics: print / set the current desktop wallpaper (Windows).
    if args.iter().any(|a| a == "--getwp") { // 条件分支
        match wallpaper::current_wallpaper() { // 分支匹配
            Some(p) => println!("{}", p.display()), // 匹配分支
            None => println!("(unknown)"), // 匹配分支
        } // 块结束
        return Ok(()); // 返回
    } // 块结束
    if args.iter().any(|a| a == "--managers") { // 条件分支
        let m = wallpaper::conflicting_wallpaper_managers(); // 变量 m
        if m.is_empty() { // 条件分支
            println!("none"); // 语句
        } else { // 语句
            for x in m { // 循环
                println!("{x}"); // 语句
            } // 块结束
        } // 块结束
        return Ok(()); // 返回
    } // 块结束
    if let Some(pos) = args.iter().position(|a| a == "--apply") { // 条件分支
        if let Some(p) = args.get(pos + 1) { // 条件分支
            match wallpaper::set_wallpaper(std::path::Path::new(p), wallpaper::Fit::Fill) { // 分支匹配
                Ok(_) => println!("APPLY OK"), // 匹配分支
                Err(e) => println!("APPLY ERR: {e}"), // 匹配分支
            } // 块结束
            println!("READBACK: {:?}", wallpaper::current_wallpaper()); // 语句
            return Ok(()); // 返回
        } // 块结束
    } // 块结束
    // Screen-size adaptation screenshots: `--shots <dir>`.
    if let Some(pos) = args.iter().position(|a| a == "--shots") { // 条件分支
        if let Some(dir) = args.get(pos + 1) { // 条件分支
            std::env::set_var("WP_SHOT_DIR", dir); // 调用 set_var
        } else { // 语句
            eprintln!("--shots requires an output directory"); // 语句
            std::process::exit(2); // 调用 exit
        } // 块结束
    } // 块结束

    let native = wallpaper_rs::native_options(); // 变量 native
    eframe::run_native( // 调用 run_native
        "wallpaper-rs", // 语句
        native, // 语句
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))), // 字段 Box
    ) // 块结束
} // 块结束

fn selftest() -> i32 { // 函数 selftest
    let mut ok = true; // 变量 ok
    let cfg = config::Config::default(); // 变量 cfg
    match serde_json::to_string(&cfg) { // 分支匹配
        Ok(s) => match serde_json::from_str::<config::Config>(&s) { // 匹配分支
            Ok(_) => println!("[PASS] config serialize + roundtrip"), // 匹配分支
            Err(_) => { // 匹配分支
                println!("[FAIL] config roundtrip"); // 语句
                ok = false; // 语句
            } // 块结束
        }, // 块结束
        Err(_) => { // 匹配分支
            println!("[FAIL] config serialize"); // 语句
            ok = false; // 语句
        } // 块结束
    } // 块结束
    if image::guess_format(&[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A]).is_ok() { // 条件分支
        println!("[PASS] image decoder available"); // 语句
    } else { // 语句
        println!("[FAIL] image decoder"); // 语句
        ok = false; // 语句
    } // 块结束
    println!("[INFO] config dir: {}", config::config_dir().display()); // 语句
    println!("[INFO] platform: {}", std::env::consts::OS); // 语句
    let (pw, ph) = wallpaper::primary_resolution(); // 调用 let
    println!( // 语句
        "[INFO] monitors: {} primary: {}x{}", // 语句
        wallpaper::monitor_count(), // 字段 wallpaper
        pw, // 语句
        ph // 语句
    ); // 块结束
    if ok { // 条件分支
        0 // 语句
    } else { // 语句
        1 // 语句
    } // 块结束
} // 块结束
