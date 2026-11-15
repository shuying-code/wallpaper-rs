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
use jni::{jni_sig, jni_str, sys, Env, JValue, JavaVM};
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
            let t = env.get_field(&bars, jni_str!("top"), jni_sig!("I"))?.i()?;
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
    if !path.is_file() {
        return Err(format!("壁纸文件不存在：{}", path.display()));
    }
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

/// Set a video as the live wallpaper.
///
/// Android has **no** `WallpaperManager.setVideoWallpaper`-style API: live
/// wallpapers are provided by a `WallpaperService` (`VideoWallpaperService` in
/// this APK) and applying one always goes through the system preview where the
/// user taps “设为壁纸”. So we:
///
/// 1. copy the video to the stable app-private path
///    `files/live-wallpaper/current-video` that the service reads;
/// 2. launch `android.service.wallpaper.CHANGE_LIVE_WALLPAPER` targeting our
///    bundled `com.shuying.wallpaperrs.VideoWallpaperService`.
///
/// The service watches the file mtime, so setting another video later simply
/// replaces the file and the running wallpaper picks it up.
pub fn set_dynamic_wallpaper(path: &std::path::Path) -> Result<(), String> {
    if !path.is_file() {
        return Err(format!("动态壁纸视频不存在：{}", path.display()));
    }
    let source_dir = internal_data_dir()
        .map(|dir| dir.join("live-wallpaper"))
        .ok_or_else(|| "Android 运行环境尚未初始化".to_string())?;
    std::fs::create_dir_all(&source_dir).map_err(|e| format!("创建动态壁纸目录失败：{e}"))?;
    let dest = source_dir.join("current-video");
    // 先写临时文件再改名：壁纸服务可能正在播放旧文件，原子替换避免读到
    // 「写了一半」的内容（服务随后按 mtime 检测并重载新视频）。
    let tmp = source_dir.join("current-video.new");
    std::fs::copy(path, &tmp).map_err(|e| format!("复制动态壁纸失败：{e}"))?;
    if let Err(e) = std::fs::rename(&tmp, &dest) {
        let _ = std::fs::remove_file(&tmp);
        return Err(format!("更新动态壁纸文件失败：{e}"));
    }

    with_env(move |env, app| -> jni::errors::Result<Result<(), String>> {
        let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr() as sys::jobject) };
        // Intent intent = new Intent("android.service.wallpaper.CHANGE_LIVE_WALLPAPER");
        let action = env.new_string("android.service.wallpaper.CHANGE_LIVE_WALLPAPER")?;
        let intent = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&action)],
        )?;
        // intent.putExtra("android.service.wallpaper.extra.LIVE_WALLPAPER_COMPONENT", component)
        let extra_key =
            env.new_string("android.service.wallpaper.extra.LIVE_WALLPAPER_COMPONENT")?;
        let extra_value = env
            .new_string("com.shuying.wallpaperrs/com.shuying.wallpaperrs.VideoWallpaperService")?;
        env.call_method(
            &intent,
            jni_str!("putExtra"),
            jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;"),
            &[JValue::Object(&extra_key), JValue::Object(&extra_value)],
        )?;
        // activity.startActivity(intent)：打开系统壁纸预览，用户点「设为壁纸」生效。
        env.call_method(
            &activity,
            jni_str!("startActivity"),
            jni_sig!("(Landroid/content/Intent;)V"),
            &[JValue::Object(&intent)],
        )?;
        Ok(Ok(()))
    })
    .map_err(|e| format!("设置动态壁纸失败：{e}"))
    .and_then(|r| r)
}

/// Extract one frame of a video as a PNG poster (used for the lock screen and
/// video thumbnails), via the system `MediaMetadataRetriever` — no mpv needed
/// on Android.
///
/// Tries a frame at 1s (`OPTION_CLOSEST_SYNC`, the cheapest decode), then the
/// first sync frame, then `OPTION_CLOSEST` at 1s; applies the rotation
/// metadata so portrait videos come out upright; compresses to PNG through
/// `Bitmap.compress(CompressFormat.PNG, …)`.
pub fn extract_video_poster(
    video: &std::path::Path,
    out: &std::path::Path,
) -> Result<(), String> {
    if !video.is_file() {
        return Err(format!("视频文件不存在：{}", video.display()));
    }
    if let Some(dir) = out.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建海报目录失败：{e}"))?;
    }
    let _ = std::fs::remove_file(out);
    let cleanup = out.to_path_buf();
    let video = video.to_string_lossy().to_string();
    let out = out.to_string_lossy().to_string();

    let result = with_env(move |env, _app| -> jni::errors::Result<Result<(), String>> {
        // MediaMetadataRetriever retriever = new MediaMetadataRetriever();
        let retriever = env.new_object(
            jni_str!("android/media/MediaMetadataRetriever"),
            jni_sig!("()V"),
            &[],
        )?;
        // retriever.setDataSource(video);
        let jvideo = env.new_string(&video)?;
        env.call_method(
            &retriever,
            jni_str!("setDataSource"),
            jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&jvideo)],
        )?;

        // Bitmap bmp = retriever.getFrameAtTime(timeUs, option)
        // OPTION_CLOSEST_SYNC = 1（只解关键帧，最省），OPTION_CLOSEST = 2（时间精确）。
        let mut bmp = JObject::null();
        for (time_us, option) in [(1_000_000_i64, 1_i32), (0, 1), (1_000_000, 2)] {
            let frame = env
                .call_method(
                    &retriever,
                    jni_str!("getFrameAtTime"),
                    jni_sig!("(JI)Landroid/graphics/Bitmap;"),
                    &[JValue::Long(time_us), JValue::Int(option)],
                )?
                .l()?;
            if !frame.as_raw().is_null() {
                bmp = frame;
                break;
            }
        }
        if bmp.as_raw().is_null() {
            let _ = env.call_method(&retriever, jni_str!("release"), jni_sig!("()V"), &[]);
            return Ok(Err("视频截帧失败：系统解码器未返回任何帧".to_string()));
        }

        // 竖拍视频：按 METADATA_KEY_VIDEO_ROTATION(24) 旋转，避免海报横躺。
        // （借 `Integer.parseInt` 解析元数据字符串，省去 JNI 字符串转换。）
        let rotation: f32 = env
            .call_method(
                &retriever,
                jni_str!("extractMetadata"),
                jni_sig!("(I)Ljava/lang/String;"),
                &[JValue::Int(24)],
            )
            .ok()
            .and_then(|v| v.l().ok())
            .filter(|s| !s.as_raw().is_null())
            .and_then(|s| {
                env.call_static_method(
                    jni_str!("java/lang/Integer"),
                    jni_str!("parseInt"),
                    jni_sig!("(Ljava/lang/String;)I"),
                    &[JValue::Object(&s)],
                )
                .ok()
                .and_then(|v| v.i().ok())
                .map(|d| d as f32)
            })
            .unwrap_or(0.0);
        if rotation == 90.0 || rotation == 270.0 {
            // Matrix matrix = new Matrix(); matrix.postRotate(rotation);
            let matrix = env.new_object(
                jni_str!("android/graphics/Matrix"),
                jni_sig!("()V"),
                &[],
            )?;
            env.call_method(
                &matrix,
                jni_str!("postRotate"),
                jni_sig!("(F)V"),
                &[JValue::Float(rotation)],
            )?;
            let w = env
                .call_method(&bmp, jni_str!("getWidth"), jni_sig!("()I"), &[])?
                .i()?;
            let h = env
                .call_method(&bmp, jni_str!("getHeight"), jni_sig!("()I"), &[])?
                .i()?;
            // Bitmap.createBitmap(bmp, 0, 0, w, h, matrix, true)
            let rotated = env
                .call_static_method(
                    jni_str!("android/graphics/Bitmap"),
                    jni_str!("createBitmap"),
                    jni_sig!(
                        "(Landroid/graphics/Bitmap;IIIILandroid/graphics/Matrix;Z)Landroid/graphics/Bitmap;"
                    ),
                    &[
                        JValue::Object(&bmp),
                        JValue::Int(0),
                        JValue::Int(0),
                        JValue::Int(w),
                        JValue::Int(h),
                        JValue::Object(&matrix),
                        JValue::Bool(true),
                    ],
                )?
                .l()?;
            if !rotated.as_raw().is_null() {
                bmp = rotated;
            }
        }

        // FileOutputStream fos = new FileOutputStream(out);
        let jout = env.new_string(&out)?;
        let fos = env.new_object(
            jni_str!("java/io/FileOutputStream"),
            jni_sig!("(Ljava/lang/String;)V"),
            &[JValue::Object(&jout)],
        )?;
        // bmp.compress(Bitmap.CompressFormat.PNG, 100, fos)
        let png = env
            .get_static_field(
                jni_str!("android/graphics/Bitmap$CompressFormat"),
                jni_str!("PNG"),
                jni_sig!("Landroid/graphics/Bitmap$CompressFormat;"),
            )?
            .l()?;
        let ok = env
            .call_method(
                &bmp,
                jni_str!("compress"),
                jni_sig!("(Landroid/graphics/Bitmap$CompressFormat;ILjava/io/OutputStream;)Z"),
                &[JValue::Object(&png), JValue::Int(100), JValue::Object(&fos)],
            )?
            .z()?;
        env.call_method(&fos, jni_str!("flush"), jni_sig!("()V"), &[])?;
        env.call_method(&fos, jni_str!("close"), jni_sig!("()V"), &[])?;
        let _ = env.call_method(&retriever, jni_str!("release"), jni_sig!("()V"), &[]);

        if ok {
            Ok(Ok(()))
        } else {
            Ok(Err("视频截帧失败：PNG 写入失败".to_string()))
        }
    })
    .map_err(|e| format!("视频截帧失败：{e}"))
    .and_then(|r| r);
    if result.is_err() {
        let _ = std::fs::remove_file(&cleanup);
    }
    result
}
