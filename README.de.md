# wallpaper-rs

<p align="center">🌐 <b>Language</b>：<a href="README.md">简体中文</a> · <a href="README.en.md">English</a> · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.pt-BR.md">Português</a></p>

Eine **plattformübergreifende Desktop-Hintergrundbild-App** mit **Rust + egui/eframe**
(Windows / macOS / Linux). Eine Oberfläche deckt **festes Hintergrundbild** und
**Diashow (zeitgesteuerter Wechsel)** ab. **Vollständig lokal: kein Login, kein
Abo, keine Telemetrie.**

<p>
  <img alt="build" src="https://github.com/shuying-code/wallpaper-rs/actions/workflows/build.yml/badge.svg">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.75%2B-orange">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-2f6fed">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green">
</p>

![Bibliothek](assets/screenshot-library.png)

---

## Funktionen

- **Bibliothek**: lokalen Ordner scannen (inkl. Unterordner), Miniaturansichten im Raster, Suche nach Dateiname.
- **Festes Hintergrundbild**: Klick auf Miniatur oder „Als Hintergrund“ setzt es sofort.
- **Diashow**: automatischer Wechsel im Intervall, sequenziell oder zufällig, mit Fortschrittsbalken.
- **Anzeigemodus**: Füllen / Anpassen / Strecken / Zentrieren / Kacheln (Windows über Registry).
- **Theme**: Hell / Dunkel mit sofortiger Anwendung.
- **Persistente Konfiguration**: Ordner, Modus, Intervall, Anzeige, Theme, letztes Bild.
- **Tastenkürzel**: `←` zurück, `→` weiter.
- **Einzelne Binärdatei, reines Rust**: keine WebView/Runtime, CJK-Schrift eingebettet.

## Ansichten

| Bibliothek | Diashow |
|---|---|
| ![Bibliothek](assets/screenshot-library.png) | ![Diashow](assets/screenshot-slideshow.png) |

## Plattformen

| Plattform | Status | Hintergrund setzen |
|---|---|---|
| Windows x64 | ✅ Gebaut & geprüft | `SystemParametersInfoW` + Registry |
| macOS | ✅ Quellcode-Support | `osascript` (System Events) |
| Linux | ✅ Quellcode-Support | GNOME `gsettings` oder `feh` |

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
