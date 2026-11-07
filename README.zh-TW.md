# wallpaper-rs

<p align="center">🌐 <b>Language</b>：<a href="README.md">简体中文</a> · <a href="README.en.md">English</a> · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.pt-BR.md">Português</a></p>

以 **Rust + egui/eframe** 编写的**跨平台桌面桌布軟體**（Windows / macOS / Linux），
一套介面即可完成 **固定桌布** 與 **輪播（定時自動更換）** 兩種模式。
**完全本機執行，無登入、無訂閱、無遙測。**

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

- **桌布庫**：從本機資料夾批次掃描圖片（可含子資料夾），以縮圖網格瀏覽，並可依檔名搜尋。
- **固定桌布**：點擊縮圖或「設為桌布」立即套用。
- **輪播模式**：依設定間隔自動更換，支援順序 / 隨機，並以進度條顯示距下次更換的剩餘時間。
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
| Windows x64 | ✅ 已建置驗證 | `SystemParametersInfoW` + 登錄檔 `Control Panel\Desktop` |
| macOS | ✅ 原始碼支援 | `osascript` 呼叫 System Events |
| Linux | ✅ 原始碼支援 | GNOME `gsettings`，或回退 `feh` |

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
