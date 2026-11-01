# wallpaper-rs

<p align="center">🌐 <b>Language</b>：<a href="README.md">简体中文</a> · <a href="README.en.md">English</a> · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.pt-BR.md">Português</a></p>

Una **aplicación de fondos de escritorio multiplataforma** hecha con **Rust + egui/eframe**
(Windows / macOS / Linux). Una sola interfaz cubre los modos **fondo fijo** y
**presentación (rotación por tiempo)**. **Funciona totalmente en local: sin inicio
de sesión, sin suscripción, sin telemetría.**

<p>
  <img alt="build" src="https://github.com/shuying-code/wallpaper-rs/actions/workflows/build.yml/badge.svg">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.75%2B-orange">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-2f6fed">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green">
</p>

![Biblioteca](assets/screenshot-library.png)

---

## Funciones

- **Biblioteca**: escanea una carpeta local (con subcarpetas), vista de miniaturas en cuadrícula y búsqueda por nombre.
- **Fondo fijo**: clic en una miniatura o en «Establecer como fondo» para aplicar al instante.
- **Presentación**: cambio automático por intervalo, secuencial o aleatorio, con barra de progreso.
- **Modo de visualización**: Rellenar / Ajustar / Estirar / Centrar / Mosaico (en Windows vía registro).
- **Tema**: claro / oscuro con cambio inmediato.
- **Configuración persistente**: carpeta, modo, intervalo, visualización, tema y último fondo.
- **Atajos**: `←` anterior, `→` siguiente.
- **Un solo binario, Rust puro**: sin WebView ni runtime, fuente CJK integrada.

## Pantallas

| Biblioteca | Presentación |
|---|---|
| ![Biblioteca](assets/screenshot-library.png) | ![Presentación](assets/screenshot-slideshow.png) |

## Plataformas

| Plataforma | Estado | Cómo se aplica el fondo |
|---|---|---|
| Windows x64 | ✅ Compilado y verificado | `SystemParametersInfoW` + registro |
| macOS | ✅ Soporte en código | `osascript` (System Events) |
| Linux | ✅ Soporte en código | GNOME `gsettings` o `feh` |

## Archivo de configuración

| Plataforma | Ruta |
|---|---|
| Windows | `%APPDATA%\wallpaper-rs\config.json` |
| macOS | `~/Library/Application Support/wallpaper-rs/config.json` |
| Linux | `$XDG_CONFIG_HOME/wallpaper-rs/config.json` (por defecto `~/.config/wallpaper-rs/config.json`) |

## Compilar

Requiere Rust 1.75+ (se recomienda stable).

```bash
cargo build --release
```

## Licencia

[MIT](LICENSE) © shuying-code
