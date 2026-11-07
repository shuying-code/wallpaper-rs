# 第三方组件声明 / Third-Party Notices

本发行包**不含**任何第三方程序，仅包含 wallpaper-rs 本体（MIT 许可）。

「设为动态壁纸」「动态壁纸锁屏」功能需要一个独立的第三方命令行媒体播放器 **mpv**。
该程序**不随本发行包分发**：首次使用这些功能时，wallpaper-rs 会从上游官方发行版
**按需下载**便携版 mpv（`.7z`）到用户缓存目录
`%LOCALAPPDATA%\wallpaper-rs\mpv-runtime`，并以**独立进程**方式调用它（不链接、
不修改）。若本机已安装 mpv（PATH / 常见安装目录），则直接使用本机版本，不会下载。

- 组件：mpv（命令行媒体播放器）及其 Windows 构建。
- 许可：**GNU General Public License v2 或更高版本（GPL-2.0-or-later）**。
- 源码：
  - 上游 mpv：<https://github.com/mpv-player/mpv>
  - Windows 构建脚本：<https://github.com/shinchiro/mpv-winbuild-cmake>
- 下载地址：<https://github.com/shinchiro/mpv-winbuild-cmake/releases>
- 如需 mpv 的对应源代码，可通过上述地址获取。也可用环境变量 `WALLPAPER_MPV`
  指定已有的 mpv.exe 路径，或设置 `WALLPAPER_MPV` 指向本机安装。

> This package contains **no** third-party binaries. The optional live-wallpaper
> feature downloads **mpv** (GPL-2.0-or-later) from its upstream releases on
> demand into `%LOCALAPPDATA%\wallpaper-rs\mpv-runtime` and invokes it as a
> separate process. Sources at the URLs above.
