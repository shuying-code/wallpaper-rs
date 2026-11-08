//! Android JNI bridge.
//!
//! The desktop build never compiles this module. It exists only for the
//! `cdylib` Android entry point (`src/android.rs`) and provides the two things
//! the pure-Rust/egui UI cannot do by itself on Android:
//!
//! * **Window (system-bar) insets** — so the header is not hidden under the
//!   status bar and the bottom navigation is not covered by the on-screen
//!   navigation / gesture bar (the "底部导航被虚拟导航栏挡住" problem).
//! * **Setting the system wallpaper** (home / lock screen) through
//!   `android.app.WallpaperManager`, using the `SET_WALLPAPER` permission the
//!   APK already declares.
//!
//! Everything is best-effort: if a JNI call fails we return an error string
//! (shown in the status bar) instead of panicking, so the UI keeps working.

use std::sync::Mutex;

use jni::objects::JObject;
use jni::{jni_sig, jni_str, sys, Env, JavaVM, JValue};
use winit::platform::android::activity::AndroidApp;

/// The `AndroidApp` handle, installed once from `android_main`.
static APP: Mutex<Option<AndroidApp>> = Mutex::new(None);

/// Remember the `AndroidApp` so UI callbacks (which run on another thread) can
/// reach the Java VM and the current Activity.
pub fn install(app: AndroidApp) {
    if let Ok(mut g) = APP.lock() {
        *g = Some(app);
    }
}

fn app_snapshot() -> Option<AndroidApp> {
    APP.lock().ok().and_then(|g| g.clone())
}

/// 应用私有、可写的内部数据目录（通常是 `/data/user/0/<pkg>/files`）。
///
/// Android 进程没有桌面意义上的 `HOME` / `XDG_CONFIG_HOME`，若沿用桌面的
/// 目录约定，配置、缩略图缓存与原图下载会写到不可写路径并全部失败。这里用
/// Activity 提供的真实私有目录，保证 [`crate::config::config_dir`] 可写。
pub fn internal_data_dir() -> Option<std::path::PathBuf> {
    app_snapshot().and_then(|app| app.internal_data_path())
}

/// Run `f` on the current thread with an attached JNI environment.
fn with_env<T, F>(f: F) -> Result<T, String>
where
    F: for<'a> FnOnce(&mut Env<'a>, &AndroidApp) -> jni::errors::Result<T>,
{
    let app = app_snapshot().ok_or_else(|| "Android 运行环境尚未初始化".to_string())?;
    // SAFETY: `vm_as_ptr` returns the process-wide JavaVM, valid for the life
    // of the app.
    let jvm = unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut sys::JavaVM) };
    jvm.attach_current_thread(|env| f(env, &app))
        .map_err(|e| format!("JNI 调用失败：{e}"))
}

/// System-bar insets in **physical pixels** as `(top, bottom)`.
///
/// `top` is the status-bar height, `bottom` the navigation / gesture bar
/// height. Returns `None` when unavailable (the caller then keeps the last
/// known value / zero padding).
pub fn system_insets() -> Option<(f32, f32)> {
    with_env(|env, app| -> jni::errors::Result<(f32, f32)> {
        // SAFETY: `activity_as_ptr` is a global reference kept alive by
        // android-activity for the whole app lifetime.
        let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr() as sys::jobject) };

        let window = env
            .call_method(
                &activity,
                jni_str!("getWindow"),
                jni_sig!("()Landroid/view/Window;"),
                &[],
            )?
            .l()?;
        let decor = env
            .call_method(
                &window,
                jni_str!("getDecorView"),
                jni_sig!("()Landroid/view/View;"),
                &[],
            )?
            .l()?;
        let insets = env
            .call_method(
                &decor,
                jni_str!("getRootWindowInsets"),
                jni_sig!("()Landroid/view/WindowInsets;"),
                &[],
            )?
            .l()?;
        if insets.as_raw().is_null() {
            return Ok((0.0, 0.0));
        }

        let sdk = env
            .get_static_field(
                jni_str!("android/os/Build$VERSION"),
                jni_str!("SDK_INT"),
                jni_sig!("I"),
            )?
            .i()?;

        let (top, bottom) = if sdk >= 30 {
            // WindowInsets.Type.systemBars() == statusBars | navigationBars |
            // captionBar == 0x7.
            let bars = env
                .call_method(
                    &insets,
                    jni_str!("getInsets"),
                    jni_sig!("(I)Landroid/graphics/Insets;"),
                    &[JValue::Int(0x7)],
                )?
                .l()?;
            let t = env
                .get_field(&bars, jni_str!("top"), jni_sig!("I"))?
                .i()?;
            let b = env
                .get_field(&bars, jni_str!("bottom"), jni_sig!("I"))?
                .i()?;
            (t, b)
        } else {
            let t = env
                .call_method(
                    &insets,
                    jni_str!("getSystemWindowInsetTop"),
                    jni_sig!("()I"),
                    &[],
                )?
                .i()?;
            let b = env
                .call_method(
                    &insets,
                    jni_str!("getSystemWindowInsetBottom"),
                    jni_sig!("()I"),
                    &[],
                )?
                .i()?;
            (t, b)
        };
        Ok((top.max(0) as f32, bottom.max(0) as f32))
    })
    .ok()
}

/// `WallpaperManager.FLAG_SYSTEM`
const FLAG_SYSTEM: i32 = 1;
/// `WallpaperManager.FLAG_LOCK`
const FLAG_LOCK: i32 = 2;

/// Set the system wallpaper from a readable image file.
///
/// `lock` selects the lock screen instead of the home screen. `fit` is ignored
/// on Android (the system applies its own crop / zoom).
pub fn set_wallpaper(path: &std::path::Path, lock: bool) -> Result<(), String> {
    let path = path.to_string_lossy().to_string();
    with_env(move |env, app| -> jni::errors::Result<Result<(), String>> {
        // SAFETY: as above — a global reference valid for the app lifetime.
        let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr() as sys::jobject) };

        // (WallpaperManager) activity.getSystemService(Context.WALLPAPER_SERVICE)
        let service = env.new_string("wallpaper")?;
        let wm = env
            .call_method(
                &activity,
                jni_str!("getSystemService"),
                jni_sig!("(Ljava/lang/String;)Ljava/lang/Object;"),
                &[JValue::Object(&service)],
            )?
            .l()?;
        if wm.as_raw().is_null() {
            return Ok(Err("Android 系统未提供 WallpaperManager".to_string()));
        }

        // InputStream in = new FileInputStream(path)
        let jpath = env.new_string(&path)?;
        let stream = env.new_object(
            jni_str!("java/io/FileInputStream"),
            jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&jpath)],
        )?;

        // wm.setStream(in, null, true, which)
        let null = JObject::null();
        let which = if lock { FLAG_LOCK } else { FLAG_SYSTEM };
        env.call_method(
            &wm,
            jni_str!("setStream"),
            jni_sig!("(Ljava/io/InputStream;Landroid/graphics/Rect;ZI)I"),
            &[
                JValue::Object(&stream),
                JValue::Object(&null),
                JValue::Bool(true),
                JValue::Int(which),
            ],
        )?;
        Ok(Ok(()))
    })
    .map_err(|e| format!("设置壁纸失败：{e}"))
    .and_then(|r| r)
}
