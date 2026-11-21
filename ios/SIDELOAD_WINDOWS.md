# Windows 签名侧载 iPhone Air

这条路线不需要本地 Mac，但构建必须在 macOS 上进行。GitHub Actions 负责生成未签名 IPA，
Windows 只负责用 Apple ID 重新签名并安装。

## 1. 触发云构建

1. 将 `ios/` 和 `.github/workflows/ios.yml` 推到 GitHub。
2. 打开仓库的 `Actions`。
3. 选择 `iOS Build` → `Run workflow`。
4. 构建完成后下载 `wallpaper-studio-ios-unsigned` 工件并解压，得到
   `WallpaperStudio-unsigned.ipa`。

## 2. 准备 iPhone Air

1. 解锁手机，用 USB-C 数据线连接 Windows。
2. 手机上点“信任此电脑”，输入锁屏密码。
3. 资源管理器能看到 `Apple iPhone → Internal Storage`，说明 USB 和文件传输正常。
   这还不等于 Sideloadly 能识别设备。
4. 安装苹果官网的 Web 版 iTunes 64 位
   <https://www.apple.com/itunes/download/win64>。安装包会提供 Sideloadly 需要的
   Apple Mobile Device Support。Microsoft Store 版通常不包含这部分驱动，不能替代。
5. 安装完成后重启 Windows，重新插线、解锁手机，并先打开 iTunes 确认能看到设备。

## 3. 在 Windows 签名安装

1. 从官方站点 <https://sideloadly.io/> 安装 Sideloadly。
2. 打开 Sideloadly，在 `iDevice` 下拉框里选择 iPhone Air；看不到设备就先回上一步检查
   iTunes 和 Apple Mobile Device Support。
3. 把 `WallpaperStudio-unsigned.ipa` 拖进 Sideloadly。
4. 输入自己的 Apple ID；开启双重验证的账号按提示输入验证码。
5. 点击 Start，等待签名和安装完成。

不要把 Apple ID 密码交给别人，也不需要把密码写进仓库或脚本。
如果提示 Bundle ID 被占用，在 Sideloadly 的包名选项里加一个唯一后缀，例如
`com.shuying.wallpaperrs.你的名字`。

## 4. 首次启动

1. 如果 App 提示需要开发者模式或安装后无法打开，进入
   `设置 → 隐私与安全性 → 开发者模式`。
2. 新 iPhone 上这个入口可能暂时隐藏：保持 USB 连接，在 Sideloadly 里再执行一次安装或
   Refresh，然后重新查看；仍没有就重启手机后查看。
3. 打开开发者模式开关，重启 iPhone，输入锁屏密码确认。
4. 如果首次打开提示“不受信任的开发者”，到
   `设置 → 通用 → VPN 与设备管理`，选择对应开发者并点“信任”。
5. 再打开 Wallpaper Studio，允许“添加照片”权限。

## 5. 注意事项

- 免费 Apple ID 的签名通常只有 7 天；到期后重新用 Sideloadly 安装同一 IPA。
- 免费账号同一台设备最多保留 3 个侧载 App。
- 这不是 App Store 安装；Apple ID 签名失效后 App 会无法启动。
- iOS 不允许第三方 App 直接修改墙纸，保存图片后仍需到“照片”里手动设置。
