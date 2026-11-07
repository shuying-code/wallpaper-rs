# wallpaper-rs

<p align="center">🌐 <b>Language</b>：<a href="README.md">简体中文</a> · <a href="README.en.md">English</a> · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.pt-BR.md">Português</a></p>

**Rust + egui/eframe** で作られた**クロスプラットフォームのデスクトップ壁紙アプリ**（Windows / macOS / Linux）。
1 つの UI で **固定壁紙**・**タイマー式スライドショー**・**ライブ壁紙**（動画）を利用できます。
既定で **「オンラインライブラリ」** を開きます。**ログイン・サブスク・テレメトリなし。**

<p>
  <img alt="build" src="https://github.com/shuying-code/wallpaper-rs/actions/workflows/build.yml/badge.svg">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.75%2B-orange">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-2f6fed">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green">
</p>

![オンラインライブラリ](assets/screenshot-online.png)
![ライブラリ](assets/screenshot-library.png)

---

## 機能

- **オンラインライブラリ（マルチサイト、既定のホーム）**：Bing 日替わり 4K・Netbian・MoeWalls・MotionBGs の 4 サイト。検索 / カテゴリ / ページ送り / お気に入りに対応し、原図のダウンロード、壁紙・ロック画面への設定が可能。ライブ系サイトは MP4 のダウンロードや**ライブ壁紙の設定**もできます。
- **ライブ壁紙（Windows / macOS / Linux-X11）**：デスクトップ アイコン層の下に自前ホストを用意し、mpv で MP4 をループ再生。**ライブ壁紙はスライドショーにも参加できます**。mpv は同梱されません——Windows は初回に**ミラーの速度を測定して最速から自動ダウンロード**、macOS / Linux は mpv のインストールが必要です。
- **ライブラリ**：ローカルフォルダを走査（サブフォルダ対応）して**画像と動画**をサムネイル表示、ファイル名で検索。動画はライブ壁紙／ロック画面に設定でき、スライドショーにも追加できます。
- **固定壁紙**：サムネイルまたは「壁紙に設定」をクリックで即適用。
- **スライドショー**：指定間隔で自動切り替え、順序 / ランダム対応、次回までの残り時間をプログレスバーで表示。**スライドショーモードなら起動時に自動で開始し、開いた瞬間に 1 枚切り替えます。**
- **表示モード**：フィル / フィット / ストレッチ / 中央 / タイル（Windows はレジストリで設定）。
- **テーマ**：ライト / ダークを即時切替。
- **設定の保存**：フォルダ、モード、間隔、表示モード、テーマ、最後の壁紙を自動保存。
- **ショートカット**：`←` 前へ、`→` 次へ。
- **単一バイナリ・純 Rust**：WebView やランタイム不要。CJK フォント同梱。
- **ログイン時に自動起動**：ワンクリック切替。Windows は Run レジストリ、macOS は LaunchAgent、Linux は autostart。

## 画面

| ライブラリ | スライドショー |
|---|---|
| ![ライブラリ](assets/screenshot-library.png) | ![スライドショー](assets/screenshot-slideshow.png) |

## 対応プラットフォーム

| プラットフォーム | 状態 | 壁紙設定の実装 |
|---|---|---|
| Windows x64 | ✅ ビルド検証済み | `SystemParametersInfoW` + レジストリ；**ライブ壁紙**：アイコン層の下に自前ホスト + mpv |
| macOS | ✅ ソース対応 | `osascript`（System Events）；**ライブ壁紙**：デスクトップ層ウィンドウ + mpv |
| Linux | ✅ ソース対応 | GNOME `gsettings`、なければ `feh`；**ライブ壁紙**：X11 `_NET_WM_WINDOW_TYPE_DESKTOP` + mpv（Wayland 非対応） |

## 設定ファイル

| プラットフォーム | パス |
|---|---|
| Windows | `%APPDATA%\wallpaper-rs\config.json` |
| macOS | `~/Library/Application Support/wallpaper-rs/config.json` |
| Linux | `$XDG_CONFIG_HOME/wallpaper-rs/config.json`（既定 `~/.config/wallpaper-rs/config.json`） |

## ビルド

Rust 1.75+（stable 推奨）。

```bash
cargo build --release
```

## ライセンス

[MIT](LICENSE) © shuying-code
