# wallpaper-rs

<p align="center">🌐 <b>Language</b>：<a href="README.md">简体中文</a> · <a href="README.en.md">English</a> · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.pt-BR.md">Português</a></p>

**Кроссплатформенное приложение для обоев рабочего стола** на **Rust + egui/eframe**
(Windows / macOS / Linux). В одном интерфейсе — **фиксированные обои** и
**слайд-шоу (автосмена по таймеру)**. **Работает полностью локально: без входа,
подписок и телеметрии.**

<p>
  <img alt="build" src="https://github.com/shuying-code/wallpaper-rs/actions/workflows/build.yml/badge.svg">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.75%2B-orange">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-2f6fed">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green">
</p>

![Библиотека](assets/screenshot-library.png)

---

## Возможности

- **Библиотека**: сканирование локальной папки (с подпапками), сетка миниатюр, поиск по имени файла.
- **Фиксированные обои**: клик по миниатюре или «Сделать обоями» — мгновенно применяется.
- **Слайд-шоу**: автосмена по интервалу, по порядку или случайно, с индикатором до следующей смены.
- **Режим отображения**: Заполнение / Вписать / Растянуть / По центру / Мозаика (в Windows — через реестр).
- **Тема**: светлая / тёмная, переключение на лету.
- **Сохранение настроек**: папка, режим, интервал, отображение, тема, последние обои.
- **Горячие клавиши**: `←` предыдущие, `→` следующие.
- **Один бинарник, чистый Rust**: без WebView и рантайма, встроенный CJK-шрифт.

## Экраны

| Библиотека | Слайд-шоу |
|---|---|
| ![Библиотека](assets/screenshot-library.png) | ![Слайд-шоу](assets/screenshot-slideshow.png) |

## Платформы

| Платформа | Статус | Установка обоев |
|---|---|---|
| Windows x64 | ✅ Собрано и проверено | `SystemParametersInfoW` + реестр |
| macOS | ✅ Поддержка в коде | `osascript` (System Events) |
| Linux | ✅ Поддержка в коде | GNOME `gsettings` или `feh` |

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
