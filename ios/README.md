# wallpaper-rs iOS

这是 `wallpaper-rs` 的原生 iPhone / iPad 入口，目标是让 Windows 开发者也能通过
GitHub Actions 的 macOS runner 构建未签名 IPA，再用 Windows 上的 Apple ID 签名工具侧载。

## 当前能力

- 必应每日壁纸（4K）
- 必应 4K 图库、关键词搜索和分页
- 原图下载并保存到系统相册
- iPhone Air 安全区、触控尺寸、纵向布局

iOS 不允许第三方 App 直接修改系统墙纸，因此 iOS 端的最终步骤一定是“保存到相册后在
照片 App 里手动设为墙纸”。这不是签名或权限问题，无法通过侧载绕过。

## 构建

需要 macOS + Xcode + [XcodeGen](https://github.com/yonaskolb/XcodeGen)：

```bash
cd ios
brew install xcodegen
xcodegen generate
open WallpaperStudio.xcodeproj
```

在 Xcode 的 Signing & Capabilities 里选择自己的 Team，连接 iPhone 后运行。

## Windows + 云构建 + 侧载

1. 将本目录和 `.github/workflows/ios.yml` 推送到 GitHub。
2. 在仓库 Actions 里手动运行 `iOS Build`。
3. 下载 `WallpaperStudio-unsigned.ipa`。
4. 在 Windows 安装苹果官网 Web 版 iTunes 64 位和 [Sideloadly](https://sideloadly.io/)，
   用 iPhone Air 的 USB-C 数据线连接。
5. Sideloadly 选择 IPA，输入 Apple ID 完成签名并安装。免费 Apple ID 通常每 7 天需要重新签名。
6. 首次启动前，在 iPhone 的“设置 → 隐私与安全性 → 开发者模式”打开开发者模式并重启。

GitHub Actions 的 macOS runner 只负责编译，不能直接连接你的手机。安装和签名必须在 Windows
（或另一台 Mac）完成。

## 后续

当前 MVP 使用 SwiftUI + URLSession 实现，不把 eframe/egui 强行搬到 iOS。这样能避免桌面依赖
和 iOS 事件循环的兼容问题，也符合 iOS 上“只浏览、保存到相册”的实际能力边界。
