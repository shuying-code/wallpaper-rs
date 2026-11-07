# wallpaper-rs

<p align="center">🌐 <b>Language</b>：<a href="README.md">简体中文</a> · <a href="README.en.md">English</a> · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.pt-BR.md">Português</a></p>

以 **Rust + egui/eframe** 编写的**跨平台桌面桌布軟體**（Windows / macOS / Linux），
一套介面即可完成 **固定桌布**、**定時輪播** 與 **動態桌布**（影片）。
預設進入 **「網路桌布庫」**；**無登入、無訂閱、無遙測。**

<p>
  <img alt="build" src="https://github.com/shuying-code/wallpaper-rs/actions/workflows/build.yml/badge.svg">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.75%2B-orange">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-2f6fed">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green">
</p>

![網路桌布庫](assets/screenshot-online.png)
![桌布庫](assets/screenshot-library.png)

---

## 功能

- **網路桌布庫（多站點線上，預設首頁）**：必應每日 4K · 彼岸圖網 · MoeWalls · MotionBGs 四個站點，支援搜尋 / 分類 / 翻頁 / 收藏；可下載原圖、設為桌布或鎖屏，動圖站還能下載 MP4 或**設為動態桌布**。
- **動態桌布（Windows / macOS / Linux-X11）**：在桌面圖示層之下自建宿主，用 mpv 循環播放 MP4；**動態桌布也能參與輪播**。mpv 不隨包分發——Windows 首次使用時**測速選最快鏡像按需下載**，macOS / Linux 需本機已安裝 mpv。
- **桌布庫**：從本機資料夾批次掃描**圖片與影片**（可含子資料夾），以縮圖網格瀏覽，並可依檔名搜尋；影片可設為動態桌布 / 鎖屏，也可加入輪播。
- **固定桌布**：點擊縮圖或「設為桌布」立即套用。
- **輪播模式**：依設定間隔自動更換，支援順序 / 隨機，並以進度條顯示距下次更換的剩餘時間。**開啟程式若為輪播模式會自動開始，並在開啟瞬間先換一張。**
- **顯示方式**：填滿 / 適應 / 拉伸 / 置中 / 並排（Windows 透過登錄檔寫入桌面樣式）。
- **主題**：淺色 / 深色一鍵切換，樣式即時生效。
- **設定持久化**：資料夾、模式、間隔、顯示方式、主題、上次桌布自動儲存。
- **鍵盤快速鍵**：`←` 上一張，`→` 下一張。
- **單一執行檔、純 Rust**：無需 WebView 或執行環境；內嵌中文字型，無缺字方框。
- **開機自動啟動**：一鍵開關，Windows 寫入 Run 登錄檔、macOS 寫入 LaunchAgent、Linux 寫入 autostart。

## 介面

| 桌布庫 | 輪播 |
|---|---|
| ![桌布庫](assets/screenshot-library.png) | ![輪播](assets/screenshot-slideshow.png) |

## 支援平台

| 平台 | 狀態 | 設定桌布的實作 |
|---|---|---|
| Windows x64 | ✅ 已建置驗證 | `SystemParametersInfoW` + 登錄檔 `Control Panel\Desktop`；**動態桌布**：圖示層之下自建宿主 + mpv |
| macOS | ✅ 原始碼支援 | `osascript` 呼叫 System Events；**動態桌布**：桌面層視窗 + mpv |
| Linux | ✅ 原始碼支援 | GNOME `gsettings`，或回退 `feh`；**動態桌布**：X11 `_NET_WM_WINDOW_TYPE_DESKTOP` + mpv（不支援 Wayland） |

## 設定檔位置

| 平台 | 路徑 |
|---|---|
| Windows | `%APPDATA%\wallpaper-rs\config.json` |
| macOS | `~/Library/Application Support/wallpaper-rs/config.json` |
| Linux | `$XDG_CONFIG_HOME/wallpaper-rs/config.json`（預設 `~/.config/wallpaper-rs/config.json`） |

## 建置

需 Rust 1.75+（建議 stable）。

```bash
cargo build --release
```

產物：`target/release/wallpaper-rs`（Windows 為 `wallpaper-rs.exe`）。

## 授權

[MIT](LICENSE) © shuying-code
