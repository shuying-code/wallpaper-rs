# wallpaper-rs

<p align="center">🌐 <b>Language</b>：<a href="README.md">简体中文</a> · <a href="README.en.md">English</a> · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.pt-BR.md">Português</a></p>

Eine **plattformübergreifende Desktop-Hintergrundbild-App** mit **Rust + egui/eframe**
(Windows / macOS / Linux). Eine Oberfläche deckt **festes Hintergrundbild**, **zeitgesteuerte
Diashow** und **Live-Hintergrundbilder** (Video) ab. Standardmäßig öffnet die
**Online-Bibliothek**; **kein Login, kein Abo, keine Telemetrie.**

<p>
  <img alt="build" src="https://github.com/shuying-code/wallpaper-rs/actions/workflows/build.yml/badge.svg">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.75%2B-orange">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-2f6fed">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green">
</p>

![Online-Bibliothek](assets/screenshot-online.png)
![Bibliothek](assets/screenshot-library.png)

---

## Funktionen

- **Online-Bibliothek (mehrere Quellen, Startseite)**: Bing-Tagesbild 4K · Netbian · MoeWalls · MotionBGs — vier Quellen. Suche / Kategorien / Seiten / Favoriten; Original herunterladen, als Hintergrund oder Sperrbildschirm setzen. Live-Quellen bieten zusätzlich MP4-Download und **„Als Live-Hintergrund setzen“**.
- **Live-Hintergrund (Windows / macOS / Linux-X11)**: ein eigener Host **hinter den Desktop-Symbolen**, mpv spielt das MP4 in Schleife. **Live-Hintergründe nehmen auch an der Diashow teil.** mpv wird nicht mitgeliefert — unter Windows wird beim ersten Mal **die Geschwindigkeit mehrerer Mirrors gemessen und der schnellste geladen**; unter macOS / Linux ist mpv erforderlich.
- **Bibliothek**: scannt einen lokalen Ordner (inkl. Unterordner) und zeigt **Bilder und Videos** als Raster mit Dateinamensuche. Videos lassen sich als Live-Hintergrund / Sperrbildschirm setzen oder zur Diashow hinzufügen.
- **Festes Hintergrundbild**: Klick auf Miniatur oder „Als Hintergrund“ setzt es sofort.
- **Diashow**: automatischer Wechsel im Intervall, sequenziell oder zufällig, mit Fortschrittsbalken. **Im Diashow-Modus startet sie beim Öffnen automatisch und wechselt sofort ein Bild.**
- **Anzeigemodus**: Füllen / Anpassen / Strecken / Zentrieren / Kacheln (Windows über Registry).
- **Theme**: Hell / Dunkel mit sofortiger Anwendung.
- **Persistente Konfiguration**: Ordner, Modus, Intervall, Anzeige, Theme, letztes Bild.
- **Tastenkürzel**: `←` zurück, `→` weiter.
- **Einzelne Binärdatei, reines Rust**: keine WebView/Runtime, CJK-Schrift eingebettet.
- **Autostart bei Anmeldung**: ein Schalter — Run-Registrierung (Windows), LaunchAgent (macOS), autostart (Linux).

## Ansichten

| Bibliothek | Diashow |
|---|---|
| ![Bibliothek](assets/screenshot-library.png) | ![Diashow](assets/screenshot-slideshow.png) |

## Plattformen

| Plattform | Status | Hintergrund setzen |
|---|---|---|
| Windows x64 | ✅ Gebaut & geprüft | `SystemParametersInfoW` + Registry; **Live-Hintergrund**: Host hinter den Symbolen + mpv |
| macOS | ✅ Quellcode-Support | `osascript` (System Events); **Live-Hintergrund**: Fenster auf Desktop-Ebene + mpv |
| Linux | ✅ Quellcode-Support | GNOME `gsettings` oder `feh`; **Live-Hintergrund**: X11 `_NET_WM_WINDOW_TYPE_DESKTOP` + mpv (Wayland nicht unterstützt) |

## Konfigurationsdatei

| Plattform | Pfad |
|---|---|
| Windows | `%APPDATA%\wallpaper-rs\config.json` |
| macOS | `~/Library/Application Support/wallpaper-rs/config.json` |
| Linux | `$XDG_CONFIG_HOME/wallpaper-rs/config.json` (Standard `~/.config/wallpaper-rs/config.json`) |

## Build

Erfordert Rust 1.75+ (stable empfohlen).

```bash
cargo build --release
```

## Lizenz

[MIT](LICENSE) © shuying-code
