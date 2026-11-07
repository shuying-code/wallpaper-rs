# wallpaper-rs

<p align="center">🌐 <b>Language</b>：<a href="README.md">简体中文</a> · <a href="README.en.md">English</a> · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.pt-BR.md">Português</a></p>

**Кроссплатформенное приложение для обоев рабочего стола** на **Rust + egui/eframe**
(Windows / macOS / Linux). В одном интерфейсе — **фиксированные обои**, **слайд-шоу
по таймеру** и **живые обои** (видео). По умолчанию открывается **«Онлайн-библиотека»**;
**без входа, подписок и телеметрии.**

<p>
  <img alt="build" src="https://github.com/shuying-code/wallpaper-rs/actions/workflows/build.yml/badge.svg">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.75%2B-orange">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-2f6fed">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green">
</p>

![Онлайн-библиотека](assets/screenshot-online.png)
![Библиотека](assets/screenshot-library.png)

---

## Возможности

- **Онлайн-библиотека (мультисайт, стартовый экран)**: Bing «обои дня» 4K · Netbian · MoeWalls · MotionBGs — четыре сайта. Поиск / категории / постранично / избранное; скачивание оригинала, установка на рабочий стол или экран блокировки. На «живых» сайтах также доступны скачивание MP4 и **установка живых обоев**.
- **Живые обои (Windows / macOS / Linux-X11)**: собственный хост-слой **позади значков рабочего стола**, mpv крутит MP4 по кругу. **Живые обои участвуют и в слайд-шоу.** mpv не входит в поставку — в Windows при первом запуске он **замеряет скорость зеркал и скачивает с самого быстрого**; в macOS / Linux нужен установленный mpv.
- **Библиотека**: сканирует локальную папку (с подпапками) и показывает **изображения и видео** сеткой миниатюр с поиском по имени. Видео можно сделать живыми обоями / экраном блокировки или добавить в слайд-шоу.
- **Фиксированные обои**: клик по миниатюре или «Сделать обоями» — мгновенно применяется.
- **Слайд-шоу**: автосмена по интервалу, по порядку или случайно, с индикатором до следующей смены. **В режиме слайд-шоу оно стартует при запуске и сразу меняет одно изображение.**
- **Режим отображения**: Заполнение / Вписать / Растянуть / По центру / Мозаика (в Windows — через реестр).
- **Тема**: светлая / тёмная, переключение на лету.
- **Сохранение настроек**: папка, режим, интервал, отображение, тема, последние обои.
- **Горячие клавиши**: `←` предыдущие, `→` следующие.
- **Один бинарник, чистый Rust**: без WebView и рантайма, встроенный CJK-шрифт.
- **Автозапуск при входе**: один переключатель — реестр Run (Windows), LaunchAgent (macOS), autostart (Linux).

## Экраны

| Библиотека | Слайд-шоу |
|---|---|
| ![Библиотека](assets/screenshot-library.png) | ![Слайд-шоу](assets/screenshot-slideshow.png) |

## Платформы

| Платформа | Статус | Установка обоев |
|---|---|---|
| Windows x64 | ✅ Собрано и проверено | `SystemParametersInfoW` + реестр; **живые обои**: хост-слой позади значков + mpv |
| macOS | ✅ Поддержка в коде | `osascript` (System Events); **живые обои**: окно на уровне рабочего стола + mpv |
| Linux | ✅ Поддержка в коде | GNOME `gsettings` или `feh`; **живые обои**: X11 `_NET_WM_WINDOW_TYPE_DESKTOP` + mpv (Wayland не поддерживается) |

## Файл настроек

| Платформа | Путь |
|---|---|
| Windows | `%APPDATA%\wallpaper-rs\config.json` |
| macOS | `~/Library/Application Support/wallpaper-rs/config.json` |
| Linux | `$XDG_CONFIG_HOME/wallpaper-rs/config.json` (по умолчанию `~/.config/wallpaper-rs/config.json`) |

## Сборка

Требуется Rust 1.75+ (рекомендуется stable).

```bash
cargo build --release
```

## Лицензия

[MIT](LICENSE) © shuying-code
