# 第三方组件声明 / Third-Party Notices

wallpaper-rs 本体的许可是 MIT。发行包**不附带 mpv 可执行文件**；Windows 的
Microsoft Edge WebView2 Runtime 由操作系统提供，也不随本应用再分发。程序会
链接并使用若干第三方 Rust 库，并在电视直播功能中使用从 ysp-live 适配的源码。

## mpv（按需下载，独立进程）

「动态壁纸」「视频 / 音乐播放」「电视直播」需要独立的第三方命令行媒体播放器
**mpv**。该程序**不随本发行包分发**：Windows 上首次使用相关功能时，
wallpaper-rs 可从上游官方发行版**按需下载**便携版 mpv（`.7z`）到用户缓存目录
`%LOCALAPPDATA%\wallpaper-rs\mpv-runtime`，并以**独立进程**方式调用它
（不链接、不修改）。若本机已安装 mpv（PATH / 常见安装目录），则直接使用本机版本。

- 组件：mpv（命令行媒体播放器）及其 Windows 构建。
- 许可：**GNU General Public License v2 或更高版本（GPL-2.0-or-later）**。
- 源码：
  - 上游 mpv：<https://github.com/mpv-player/mpv>
  - Windows 构建脚本：<https://github.com/shinchiro/mpv-winbuild-cmake>
- 下载地址：<https://github.com/shinchiro/mpv-winbuild-cmake/releases>
- 下载时会先并发测速官方直连与多个 GitHub 加速镜像，选最快且可用的地址；
  也可用环境变量 `WALLPAPER_MPV_URL` 指定自定义下载地址，或用
  `WALLPAPER_MPV` 指定已有的 `mpv.exe`。

## ysp-live 适配源码

电视直播的频道目录、官方页面鉴权脚本和部分桥协议 / 播放规则适配自
[ysp-live](https://github.com/shuying-code/ysp-live)。

- 使用位置：
  - `assets/live/channels.json`：64 路频道种子；
  - `assets/live/auth_bridge.js`：官方页面鉴权桥脚本；
  - `src/live/{bridge,error,model}.rs`：桥协议、错误类型与频道模型；
  - `src/live/{host,session}.rs`：针对 wallpaper-rs 进程模型的 Windows 宿主与回退实现。
- 许可：MIT。
- 版权：Copyright (c) 2026 ysp-live contributors。

完整 MIT 许可文本：

```text
MIT License

Copyright (c) 2026 ysp-live contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## WebView2 Runtime 与 Rust WebView 库

Windows 电视直播通过系统 **Microsoft Edge WebView2 Runtime** 加载央视频官方
页面，用于让官方 JavaScript 完成鉴权。Runtime 属于 Microsoft，不随本应用分发，
其使用受 Microsoft 的软件许可条款约束：
<https://learn.microsoft.com/microsoft-edge/webview2/>

用于嵌入 WebView2 的 Rust 库：

- **wry** 0.57.x：Apache-2.0 OR MIT，<https://github.com/tauri-apps/wry>
- **tao** 0.37.x：Apache-2.0，<https://github.com/tauri-apps/tao>
- **windows / webview2-com** 等 Rust 绑定：MIT OR Apache-2.0，
  由 Cargo 依赖树引入。

其余 Rust 依赖及其许可证可通过 `cargo metadata` 或 `cargo deny` 审计。各依赖的
版权归其各自贡献者所有。

## 内容与合规

电视直播的内容来自央视频官方网站。本应用只做播放器与官方页面之间的技术适配，
**不提供内容源、不代理视频、不绕过 DRM / 付费 / 会员 / 地区限制**，也不声称
拥有央视、央视频或各卫视的内容再分发授权。频道可用性、清晰度与登录要求由
官方服务决定。公开发布或商业使用前请自行取得必要授权并完成当地法律审查。

> This package contains no bundled mpv executable and does not redistribute
> the WebView2 Runtime. mpv (GPL-2.0-or-later) is downloaded on demand as a
> separate process; WebView2 is supplied by Microsoft. TV-live channel data,
> bridge script and core rules are adapted from the MIT-licensed ysp-live
> project; its complete MIT notice is reproduced above. wry is Apache-2.0 OR
> MIT and tao is Apache-2.0. Live content comes from the official Yangshipin
> website; this app provides no content source and bypasses no DRM, payment,
> membership or regional restriction.
