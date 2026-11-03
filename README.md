# wallpaper-rs

<p align="center">🌐 <b>Language</b>：<a href="README.md">简体中文</a> · <a href="README.en.md">English</a> · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.pt-BR.md">Português</a></p>

用 **Rust + egui/eframe** 编写的**跨平台桌面壁纸软件**（Windows / macOS / Linux），
一套界面即可完成 **固定壁纸** 与 **轮播（定时自动更换）** 两种模式。
**完全本地运行，无登录、无订阅、无遥测。**

<p>
  <img alt="build" src="https://github.com/shuying-code/wallpaper-rs/actions/workflows/build.yml/badge.svg">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.75%2B-orange">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-2f6fed">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green">
</p>

![壁纸库](assets/screenshot-library.png)

---

## 下载

前往 [最新 Release](https://github.com/shuying-code/wallpaper-rs/releases/latest) 下载对应平台安装包：

| 平台 | 下载 |
|---|---|
| Windows (x64) | [wallpaper-rs-windows-x64.zip](https://github.com/shuying-code/wallpaper-rs/releases/latest/download/wallpaper-rs-windows-x64.zip) |
| macOS (Universal, Intel+M 芯片) | [wallpaper-rs-macos-universal.tar.gz](https://github.com/shuying-code/wallpaper-rs/releases/latest/download/wallpaper-rs-macos-universal.tar.gz) |
| Linux / Ubuntu (x64) | [wallpaper-rs-linux-x64.tar.gz](https://github.com/shuying-code/wallpaper-rs/releases/latest/download/wallpaper-rs-linux-x64.tar.gz) |
| 全部版本 | [Releases 页面](https://github.com/shuying-code/wallpaper-rs/releases/latest) |

在应用内「关于」页也可直接点击下载。

> 移动端：**Android 已提供可安装 APK**（`wallpaper-rs-android-universal.apk`，含 arm64/armv7/x86_64），构建方式见 [MOBILE.md](MOBILE.md)。**iOS** 需 macOS + Xcode + 苹果签名，Windows 无法产出可安装包。

## 功能

- **壁纸库（按文件夹相册）**：从本地文件夹批量扫描图片（可含子文件夹），自动按所在文件夹分组为相册；每个相册可单独**展开 / 折叠**，顶部提供「全部展开 / 全部折叠」，并支持按文件名搜索过滤。
- **侧栏显隐**：点击标题栏左上角 `☰` 可一键隐藏 / 显示左侧导航栏，专注看图；状态自动记忆。
- **权限请求与授权引导**：扫描遇到无读取权限的目录时不再静默忽略，而是显示提示条，可「请求权限 / 重试」（macOS 触发系统隐私授权）、「打开文件夹位置 / 打开授权设置」跳到系统授权页面或资源管理器，由操作人确认授权。
- **固定壁纸**：点击缩略图或「设为壁纸」，立即应用到桌面。
- **轮播模式**：按设定间隔自动更换，支持顺序 / 随机播放，进度条显示距下次更换的剩余时间。
- **轮播清单（可增删）**：可在「壁纸库」卡片和「每日壁纸」上把图片「加入轮播 / 移出轮播」，自由组合本地图片与在线每日壁纸；「轮播」页展示当前清单缩略图，可随时「移除 / 设为当前」，也可一键恢复为「全部壁纸库」。
- **显示方式**：填充 / 适应 / 拉伸 / 居中 / 平铺（Windows 写注册表，Linux 通过 GNOME `picture-options`）。
- **主题**：浅色 / 深色一键切换，样式即时生效。
- **配置持久化**：文件夹、模式、间隔、显示方式、主题、上次壁纸等自动保存到本地配置文件。
- **开机自启**：一键开关，Windows 写 `Run` 注册表、macOS 写 `LaunchAgent`、Linux 写 autostart `.desktop`。
- **全盘扫描**：后台扫描全部磁盘的图片并载入壁纸库（跳过系统目录、限量收集）。
- **关闭到系统托盘（图标常显）**：点击右上角「×」不退出，而是隐藏到系统托盘（Windows 右下角通知区域 / macOS 菜单栏 / Linux 状态区），任务栏图标消失、程序与轮播继续在后台运行。程序会在 Windows 上尽量把自身托盘图标固定为「始终显示」（写入 `NotifyIconSettings` 的 `IsPromoted`），避免被 Win11 收进「显示隐藏的图标」；设置页也提供「打开托盘设置」一键手动固定。单击托盘图标即可唤回并居中显示；右键菜单「退出 wallpaper-rs」为真正的退出——不仅结束后台进程，还会一并结束本机所有其它 wallpaper-rs 实例。
- **全局快捷键**：`Ctrl+Alt+W` 一键显示 / 隐藏窗口（被占用时自动回退到 `Ctrl+Alt+Shift+W` / `Ctrl+Alt+F9`）。
- **显示适配**：检测显示器数量与主屏分辨率，高 DPI 自动适配，壁纸应用到所有显示器。
- **全新界面 + 响应式适配**：重新设计的暖色现代 UI；窗口变窄自动切换底部标签栏的紧凑布局，兼容手机 / 平板 / 桌面（iOS / Android 界面自适应）。
- **大图库分页懒加载**：单个相册图片很多时先按页渲染（每页 48 张）+「加载更多」，配合缩略图按需解码，几千张也不卡顿、不爆显存。
- **多尺寸适配截图（`--shots`）**：一条命令自动把界面在手机 / 平板 / 桌面三种尺寸下逐页截图，便于核对响应式效果。
- **在线每日壁纸**：联网获取 Bing 每日壁纸（`cn.bing.com`，国内可直接访问），支持「了解有关此图片信息」、点赞 / 踩、上一张 / 下一张、设为桌面 / 锁屏，以及每日自动更换。
- **锁屏壁纸**：把当前壁纸同步设为系统锁屏（Windows 写 `PersonalizationCSP`，点击时自动弹出 UAC 管理员授权、同意后即完成，无需手动以管理员身份运行；Ubuntu/GNOME `gsettings` 锁屏 `picture-uri`；macOS 锁屏跟随系统壁纸）。
- **跨平台全局快捷键**：Windows `RegisterHotKey`，macOS / Linux(X11) 使用 `global-hotkey`，一键显示 / 隐藏。
- **键盘快捷键**：`←` 上一张，`→` 下一张。
- **单文件、纯 Rust**：无需 WebView 或运行时，双击即用；内嵌中文字体，无缺字方框。

## 图形界面（GUI）

界面已重新设计为「暖调编辑风」：米白纸感背景 + 陶土橙主色 + 深青点缀，圆角卡片与更宽松的留白。桌面端为左侧分段导航（壁纸库 / 轮播 / 每日壁纸 / 设置 / 关于），可用标题栏 `☰` 隐藏；壁纸库按文件夹相册分组并可展开 / 折叠，底部状态栏显示载入数量与操作结果。

**响应式 / 移动端**：窗口变窄（手机 / 平板，宽度 < 820pt）时自动切换为紧凑单栏布局——左侧导航收起为底部标签栏（壁纸库 / 轮播 / 每日 / 设置 / 关于），按钮换行、搜索与计数分行、卡片按 2–3 列自适应、触控目标加大，主题切换移到标题栏。

| 壁纸库 | 轮播 |
|---|---|
| ![壁纸库](assets/screenshot-library.png) | ![轮播](assets/screenshot-slideshow.png) |

| 设置 | 关于 |
|---|---|
| ![设置](assets/screenshot-settings.png) | ![关于](assets/screenshot-about.png) |

深色主题：

![深色主题](assets/screenshot-library-dark.png)

移动端紧凑布局（底部标签栏）：

| 图库 | 设置 |
|---|---|
| ![移动图库](assets/screenshot-mobile-library.png) | ![移动设置](assets/screenshot-mobile-settings.png) |

## 支持的平台

| 平台 | 状态 | 设置壁纸的实现 |
|---|---|---|
| Windows x64 | ✅ 已构建验证 | `SystemParametersInfoW` + 注册表 `Control Panel\Desktop` |
| macOS | ✅ 源码支持 | `osascript` 调用 System Events 设置桌面图片 |
| Linux / Ubuntu | ✅ 源码支持 | GNOME/Ubuntu `gsettings`（含 `picture-options`），或回退到 `feh` |
| Android | 🧩 界面适配（需移动端工具链） | 移动宿主经 `WallpaperManager` 设置（需 `SET_WALLPAPER` 权限）；界面已做触控 / 窄屏适配 |
| iOS / iPadOS | 🧩 界面适配（仅浏览） | 系统不允许 App 直接改壁纸，需「导出到相册」后手动设为墙纸；界面已做触控 / 窄屏适配 |

桌面三平台共用同一套源码；移动端复用同一套 egui 界面并提供各平台能力说明。配置文件写入各平台的标准目录（见下）。

> 说明：系统托盘在 Windows / macOS / Linux 桌面环境（AppIndicator）下可用。全局快捷键在 Linux 上依赖 X11；Wayland 会话下系统不允许应用抢占全局热键，此时仍可通过托盘图标唤回窗口。macOS / Linux 的锁屏：Ubuntu 用 `gsettings` 设置，macOS 锁屏壁纸跟随系统。

## 配置文件位置

| 平台 | 路径 |
|---|---|
| Windows | `%APPDATA%\wallpaper-rs\config.json` |
| macOS | `~/Library/Application Support/wallpaper-rs/config.json` |
| Linux | `$XDG_CONFIG_HOME/wallpaper-rs/config.json`（默认 `~/.config/wallpaper-rs/config.json`） |

## 构建

需要 Rust 1.75+（推荐 stable）。

```bash
cargo build --release
```

产物：`target/release/wallpaper-rs`（Windows 为 `wallpaper-rs.exe`）。

### 运行

```bash
cargo run --release
```

### 自检

```bash
cargo run -- --selftest      # 配置序列化 + 图片解码器检查
cargo run -- --version       # 打印版本
```

### 多尺寸适配截图

```bash
cargo run --release -- --shots ./shots
# 自动在 手机(390×844) / 平板(834×1112) / 桌面(1440×900)
# 下逐页（图库/轮播/设置/关于）截图，保存为 shot-*.png
```

## 使用速览

1. 打开程序，进入「壁纸库」，点击「选择文件夹…」选中图片目录。
2. 点击任意缩略图即可设为固定壁纸；或进入「轮播」选择间隔与顺序后点「开始」。
3. 在「设置」里切换显示方式与主题；配置会自动保存。

> 详细图文步骤见 [docs/用户操作手册.md](docs/用户操作手册.md)。

## 目录结构

```
wallpaper-rs/
├─ src/
│  ├─ main.rs        # 入口、--selftest、窗口参数
│  ├─ app.rs         # GUI：壁纸库 / 轮播 / 设置 / 关于
│  ├─ theme.rs       # 配色、样式、系统 CJK 字体加载
│  ├─ config.rs      # 配置结构、跨平台配置目录
│  ├─ wallpaper.rs   # 各平台设置壁纸实现
│  ├─ autostart.rs   # 开机自启（Windows 注册表 / LaunchAgent / autostart）
│  └─ thumbs.rs      # 缩略图工作线程
├─ docs/             # 用户操作手册与文档索引
├─ assets/           # 截图
└─ dist/             # 本机构建产物（Windows exe）
```

## 许可

[MIT](LICENSE) © shuying-code
