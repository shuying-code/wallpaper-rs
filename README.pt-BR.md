# wallpaper-rs

<p align="center">🌐 <b>Language</b>：<a href="README.md">简体中文</a> · <a href="README.en.md">English</a> · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.pt-BR.md">Português</a></p>

Um **aplicativo de papel de parede multiplataforma** feito com **Rust + egui/eframe**
(Windows / macOS / Linux). Uma única interface cobre os modos **papel fixo** e
**apresentação (rotação por tempo)**. **Totalmente local: sem login, sem
assinatura, sem telemetria.**

<p>
  <img alt="build" src="https://github.com/shuying-code/wallpaper-rs/actions/workflows/build.yml/badge.svg">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.75%2B-orange">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-2f6fed">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green">
</p>

![Biblioteca](assets/screenshot-library.png)

---

## Recursos

- **Biblioteca**: varre uma pasta local (com subpastas), grade de miniaturas e busca por nome.
- **Papel fixo**: clique na miniatura ou em "Definir como papel de parede" para aplicar na hora.
- **Apresentação**: troca automática por intervalo, sequencial ou aleatória, com barra de progresso.
- **Modo de exibição**: Preencher / Ajustar / Esticar / Centralizar / Mosaico (Windows via registro).
- **Tema**: claro / escuro com troca imediata.
- **Configuração persistente**: pasta, modo, intervalo, exibição, tema e último papel.
- **Atalhos**: `←` anterior, `→` próximo.
- **Binário único, Rust puro**: sem WebView nem runtime, fonte CJK embutida.
- **Iniciar com o sistema**: um interruptor — registro Run (Windows), LaunchAgent (macOS), autostart (Linux).

## Telas

| Biblioteca | Apresentação |
|---|---|
| ![Biblioteca](assets/screenshot-library.png) | ![Apresentação](assets/screenshot-slideshow.png) |

## Plataformas

| Plataforma | Status | Como define o papel |
|---|---|---|
| Windows x64 | ✅ Compilado e verificado | `SystemParametersInfoW` + registro |
| macOS | ✅ Suporte no código | `osascript` (System Events) |
| Linux | ✅ Suporte no código | GNOME `gsettings` ou `feh` |

## Arquivo de configuração

| Plataforma | Caminho |
|---|---|
| Windows | `%APPDATA%\wallpaper-rs\config.json` |
| macOS | `~/Library/Application Support/wallpaper-rs/config.json` |
| Linux | `$XDG_CONFIG_HOME/wallpaper-rs/config.json` (padrão `~/.config/wallpaper-rs/config.json`) |

## Compilar

Requer Rust 1.75+ (stable recomendado).

```bash
cargo build --release
```

## Licença

[MIT](LICENSE) © shuying-code
