//! Android entry point.
//!
//! `cargo-apk` builds this crate as a `cdylib` and wraps it in a
//! `NativeActivity`; the OS calls the exported `android_main`, which hands the
//! `AndroidApp` to winit (via eframe's `event_loop_builder` hook) and then runs
//! the normal eframe/egui app.

use crate::app;
use winit::platform::android::activity::AndroidApp;
use winit::platform::android::EventLoopBuilderExtAndroid;

#[no_mangle]
pub fn android_main(app: AndroidApp) {
    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
    let mut native = crate::native_options();
    // On Android the AndroidApp must be attached while building the event loop.
    native.event_loop_builder = Some(Box::new(move |builder| {
        builder.with_android_app(app);
    }));
    if let Err(e) = eframe::run_native(
        "wallpaper-rs",
        native,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    ) {
        log::error!("eframe run_native failed: {e}");
    }
}
