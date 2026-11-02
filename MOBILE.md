# 移动端（Android / iOS / iPadOS）构建说明

## 现状

- **桌面三平台**（Windows x64 / macOS Universal / Linux x64）由 GitHub Actions 自动构建，并发布到 [Releases](https://github.com/shuying-code/wallpaper-rs/releases)。
- **界面已完成移动适配**：`src/platform.rs` 识别 Android / iOS，`is_mobile()` 强制紧凑布局；窄屏自动切换底部标签栏、触控目标加大、卡片 2–3 列自适应；Android（NotoSansCJK）/ iOS（PingFang）中文回退。
- **可安装的 Android APK / iOS IPA 尚未产出**，原因不是界面，而是**构建工具链与签名**（详见下）。本仓库提供界面与平台能力层，真正的移动端打包需要下列环境。

> 说明：iOS 的「不能以编程方式设置壁纸」是系统限制，任何 iOS 壁纸 App 都只能浏览 / 导出到相册后由用户手动设为墙纸。本项目在 iOS 上按此能力实现。

---

## Android（可构建 APK，需要工具链）

### 需要的环境
1. **JDK 17+**（本机已安装 `D:\app\jdk-17.0.20.8-hotspot`）。
2. **Android SDK**：`platform-tools`、`platforms;android-34`、`build-tools;34.0.0`。
3. **Android NDK** r26+（约 1.5 GB）。
4. Rust Android 目标：`rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`。
5. `cargo install cargo-ndk cargo-apk`（或使用 `cargo-mobile2` + Gradle 工程）。

### 代码层仍需的门控（关键）
桌面专属的 crate 在 Android 目标下需要 `#[cfg(...)]` 排除或替换，否则无法编译：
- `tray-icon`（仅 Windows/macOS/Linux）——移动端隐藏托盘功能。
- `global-hotkey`（已在 `cfg(not(windows))` 下，但仍需排除 Android）。
- `rfd` 文件对话框——移动端改用系统相册 / 文件选择器。
- `src/winhide.rs` 仅在 Windows 生效（已属 Windows 专用）。
- `src/main.rs` 入口：Android 需以 `android-activity` 的 `android_main` 承载 eframe，而不是桌面 `main()`。

### 建议步骤（安装工具链后）
```bash
# 1) 安装 SDK/NDK（可用 Android Studio 或 cmdline-tools 的 sdkmanager）
sdkmanager "platform-tools" "platforms;android-34" "build-tools;34.0.0" "ndk;26.1.10909125"

# 2) Rust 目标与工具
rustup target add aarch64-linux-android
cargo install cargo-ndk cargo-apk

# 3) 在已按上面做 cfg 门控、并配好 android 入口后构建
cargo apk build --release
# 产物：target/release/apk/wallpaper-rs.apk
```

> 若需要，我可以继续把上述 `cfg` 门控与 Android 入口（`android_main`）补进代码，并加一个 `android` 的 CI 任务；那属于一次「移动端移植」，不只是打 tag。请在可安装 SDK/NDK 的前提下确认。

---

## iOS / iPadOS（Windows 上无法构建）

iOS 可安装包**必须**在 macOS 上用 Xcode 构建，Windows 环境无法产出 `.app` / `.ipa`。需要：
1. **一台 Mac + Xcode 15+**。
2. **Apple Developer 账号**与签名材料：开发者证书（`.p12`）+ 描述文件（`.mobileprovision`），或付费开发者账号做 TestFlight 发布。免费个人账号只能用于 7 天有效期的自签名侧载。
3. Rust iOS 目标：`rustup target add aarch64-apple-ios`（真机）/ `aarch64-apple-ios-sim`（模拟器）。
4. eframe / winit 的 iOS 宿主工程（Xcode 项目里以 `UIViewController` 承载 winit 的 iOS 后端，链接 Rust 静态库）。

构建产物：
- 未签名 `.app`（仅可用于模拟器或需自行签名侧载）；
- 正式 `.ipa` 需要上述签名证书 / 描述文件。

> 因此：**没有 Mac + Xcode + 苹果签名，就无法产出可在 iPhone 上安装的 iOS 包**——这不是本项目能绕过的限制。

---

## 需要你提供（才能产出移动安装包）

| 平台 | 需要提供 |
|---|---|
| Android | 允许在本机安装 Android SDK + NDK（或告知已有路径），我将补齐 cfg 门控 + `android_main` 入口并尝试 `cargo apk build`。 |
| iOS | 一台 Mac + Xcode + Apple 签名（`.p12` + `.mobileprovision` 或开发者账号），在 macOS 上构建并提供 `.ipa`。 |

在此之前，桌面三平台的安装包已在 Releases 持续发布可用。
