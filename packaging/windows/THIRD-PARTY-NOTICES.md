# 第三方组件声明 / Third-Party Notices

本发行包内除 wallpaper-rs 本体外，还捆绑了一个独立的第三方程序 **mpv**（用于
「设为动态壁纸」功能）。wallpaper-rs 以**独立进程**方式调用它（不链接、不修改），
属于「聚合分发」，wallpaper-rs 本身仍按 MIT 许可分发。

## mpv（`mpv/` 目录）

- 组件：mpv（命令行媒体播放器）及其 Windows 构建，版本见 `mpv/mpv.exe --version`。
- 许可：**GNU General Public License v2 或更高版本（GPL-2.0-or-later）**。
  许可证全文见 `mpv/LICENSE-GPLv2.txt`。
- 源码：
  - 上游 mpv：<https://github.com/mpv-player/mpv>
  - Windows 构建脚本：<https://github.com/shinchiro/mpv-winbuild-cmake>
- 如需 mpv 的对应源代码，可通过上述地址获取。

> Bundled third-party binary: **mpv** (GPL-2.0-or-later), invoked as a separate
> process for the "Set as live wallpaper" feature. Full license text in
> `mpv/LICENSE-GPLv2.txt`; sources at the URLs above.
