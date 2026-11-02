# 移动端（Android / iOS / iPadOS）说明

## 现状

| 平台 | 状态 |
|---|---|
| Windows / macOS / Linux | ✅ 桌面安装包由 GitHub Actions 构建，发布到 [Releases](https://github.com/shuying-code/wallpaper-rs/releases)。 |
| **Android** | ✅ **可构建并签名出可安装 APK**（`wallpaper-rs-android-universal.apk`，含 armeabi-v7a / arm64-v8a / x86_64）。界面已做触控 / 窄屏适配。 |
| **iOS / iPadOS** | ⛔ 需 macOS + Xcode + 苹果签名，Windows 上无法产出可安装包（详见下文）。 |

---

## Android

代码已完成 Android 移植：新增 `src/lib.rs`（共享库）+ `src/android.rs`（`android_main` 入口），
桌面专属的 `tray-icon` / `global-hotkey` / `rfd` 已按平台 `cfg` 门控，`Cargo.toml` 增加
`[lib] crate-type = ["cdylib","rlib"]` 与 `[package.metadata.android]` 配置。

### 构建环境（已在开发机验证）
- JDK 17（`D:\app\jdk-17.0.20.8-hotspot`）
- Android SDK：`platform-tools`、`platforms;android-34`、`build-tools;34.0.0`
- Android NDK：`26.1.10909125`
- Rust 目标：`aarch64-linux-android`、`armv7-linux-androideabi`、`x86_64-linux-android`
- `cargo install cargo-apk`（0.10.0）

### 构建命令
```bash
export ANDROID_HOME=/path/to/android-sdk
export ANDROID_NDK_HOME=$ANDROID_HOME/ndk/26.1.10909125
export JAVA_HOME=/path/to/jdk17
# 签名（release 必须）：通过环境变量传入，不要把密钥写进仓库
export CARGO_APK_RELEASE_KEYSTORE=/path/to/release.jks
export CARGO_APK_RELEASE_KEYSTORE_PASSWORD=******
# 关键：加 --lib，只构建 cdylib（否则 cargo-apk 会把桌面 bin 也当 cdylib 处理而报错）
cargo apk build --release --lib
# 产物：target/release/apk/wallpaper-rs.apk
```
> 未配置签名时可去掉环境变量构建 **debug** APK（`cargo apk build --lib`，自动使用调试密钥，可直接侧载安装）。

### 安装
```bash
adb install -r wallpaper-rs-android-universal.apk
```
最低 Android 7.0（API 24），目标 API 34。已声明权限：`SET_WALLPAPER`、`INTERNET`、
`READ_EXTERNAL_STORAGE`、`READ_MEDIA_IMAGES`。

### Android 端已知限制 / 后续
- 移动端隐藏了「托盘 / 全局快捷键」等桌面专属项（无系统托盘与全局热键）。
- 选择图片目录目前仍需接入系统相册（MediaStore）选择器；当前可在设置里指定目录后扫描，
  在线「每日壁纸」（INTERNET）可直接使用。下一步可接入 Android 照片选择器以完整浏览本机图片。
- 设置壁纸通过 `WallpaperManager`（需 `SET_WALLPAPER`），已在 `src/wallpaper.rs` 的 android 分支实现。

---

## iOS / iPadOS（Windows 无法构建）

iOS 可安装包**必须**在 macOS 上用 Xcode 构建，Windows 环境无法产出 `.app` / `.ipa`。需要：
1. 一台 Mac + Xcode 15+。
2. Apple 开发者账号与签名材料（`.p12` + `.mobileprovision`，或付费账号走 TestFlight）。
3. Rust 目标 `aarch64-apple-ios`（真机）/ `aarch64-apple-ios-sim`（模拟器）。
4. eframe / winit 的 iOS 宿主工程（Xcode 里用 `UIViewController` 承载 winit iOS 后端，链接 Rust 静态库）。

> 且 iOS 系统不允许 App 直接改壁纸：iOS 端只能浏览 / 导出到相册后由用户手动设为墙纸。
> **没有 Mac + Xcode + 苹果签名就无法产出可在 iPhone 上安装的 iOS 包**，这是平台硬性限制。
