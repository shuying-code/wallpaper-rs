# wallpaper-rs

<p align="center">🌐 <b>Language</b>：<a href="README.md">简体中文</a> · <a href="README.en.md">English</a> · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.pt-BR.md">Português</a></p>

Una **aplicación de fondos de escritorio multiplataforma** hecha con **Rust + egui/eframe**
(Windows / macOS / Linux). Una sola interfaz cubre **fondo fijo**, **presentación
(rotación por tiempo)** y **fondos animados** (vídeo). Abre por defecto en la
**Biblioteca en línea**; **sin inicio de sesión, sin suscripción, sin telemetría.**

<p>
  <img alt="build" src="https://github.com/shuying-code/wallpaper-rs/actions/workflows/build.yml/badge.svg">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.75%2B-orange">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-2f6fed">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green">
</p>

![Biblioteca en línea](assets/screenshot-online.png)
![Biblioteca](assets/screenshot-library.png)

---

## Funciones

- **Biblioteca en línea (multisitio, inicio por defecto)**: Bing del día 4K · Netbian · MoeWalls · MotionBGs: cuatro sitios. Búsqueda / categorías / paginación / favoritos; descarga del original y aplicación como fondo o pantalla de bloqueo. Los sitios con vídeo permiten además descargar el MP4 o **establecer un fondo animado**.
- **Fondos animados (Windows / macOS / Linux-X11)**: un host propio **tras los iconos del escritorio**, con mpv reproduciendo el MP4 en bucle. **Los fondos animados también participan en la presentación.** mpv no se incluye — en Windows, al primer uso **mide la velocidad de varios espejos y descarga el más rápido**; en macOS / Linux se requiere mpv instalado.
- **Biblioteca**: escanea una carpeta local (con subcarpetas) y muestra **imágenes y vídeos** en cuadrícula con búsqueda por nombre. Los vídeos se pueden poner como fondo animado / pantalla de bloqueo o añadir a la presentación.
- **Fondo fijo**: clic en una miniatura o en «Establecer como fondo» para aplicar al instante.
- **Presentación**: cambio automático por intervalo, secuencial o aleatorio, con barra de progreso. **En modo presentación se inicia sola al abrir y cambia una imagen al instante.**
- **Modo de visualización**: Rellenar / Ajustar / Estirar / Centrar / Mosaico (en Windows vía registro).
- **Tema**: claro / oscuro con cambio inmediato.
- **Configuración persistente**: carpeta, modo, intervalo, visualización, tema y último fondo.
- **Atajos**: `←` anterior, `→` siguiente.
- **Un solo binario, Rust puro**: sin WebView ni runtime, fuente CJK integrada.
- **Inicio automático**: un interruptor — registro Run (Windows), LaunchAgent (macOS), autostart (Linux).

## Pantallas

| Biblioteca | Presentación |
|---|---|
| ![Biblioteca](assets/screenshot-library.png) | ![Presentación](assets/screenshot-slideshow.png) |

## Plataformas

| Plataforma | Estado | Cómo se aplica el fondo |
|---|---|---|
| Windows x64 | ✅ Compilado y verificado | `SystemParametersInfoW` + registro; **fondo animado**: host tras los iconos + mpv |
| macOS | ✅ Soporte en código | `osascript` (System Events); **fondo animado**: ventana a nivel de escritorio + mpv |
| Linux | ✅ Soporte en código | GNOME `gsettings` o `feh`; **fondo animado**: X11 `_NET_WM_WINDOW_TYPE_DESKTOP` + mpv (Wayland no soportado) |

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
