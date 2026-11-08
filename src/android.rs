//! Android entry point.
//!
//! `cargo-apk` builds this crate as a `cdylib` and wraps it in a
//! `NativeActivity`; the OS calls the exported `android_main`, which hands the
//! `AndroidApp` to winit (via eframe's `event_loop_builder` hook) and then runs
//! the normal eframe/egui app.

use crate::app; // 引入依赖
use winit::platform::android::activity::AndroidApp; // 引入依赖
use winit::platform::android::EventLoopBuilderExtAndroid; // 引入依赖

#[no_mangle] // 语句
pub fn android_main(app: AndroidApp) { // 函数 android_main
    android_logger::init_once( // 调用 init_once
        android_logger::Config::default().with_max_level(log::LevelFilter::Info), // 字段 android_logger
    ); // 块结束
    // Keep a handle for the JNI bridge (system-bar insets / WallpaperManager):
    // UI callbacks run on a different thread and must reach the Java VM.
    crate::android_jni::install(app.clone()); // 语句
    let mut native = crate::native_options(); // 变量 native
    // On Android the AndroidApp must be attached while building the event loop.
    native.event_loop_builder = Some(Box::new(move |builder| { // 语句
        builder.with_android_app(app); // 语句
    })); // 语句
    if let Err(e) = eframe::run_native( // 条件分支
        "wallpaper-rs", // 语句
        native, // 语句
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))), // 字段 Box
    ) { // 语句
        log::error!("eframe run_native failed: {e}"); // 语句
    } // 块结束
} // 块结束
