# wallpaper-rs

<p align="center">🌐 <b>Language</b>：<a href="README.md">简体中文</a> · <a href="README.en.md">English</a> · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.pt-BR.md">Português</a></p>

**Rust + egui/eframe** 로 만든 **크로스 플랫폼 데스크톱 배경화면 앱**(Windows / macOS / Linux).
하나의 UI 로 **고정 배경화면** 과 **슬라이드쇼(시간별 자동 전환)** 두 모드를 모두 지원합니다.
**완전 로컬 실행. 로그인·구독·텔레메트리 없음.**

<p>
  <img alt="build" src="https://github.com/shuying-code/wallpaper-rs/actions/workflows/build.yml/badge.svg">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.75%2B-orange">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-2f6fed">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green">
</p>

![라이브러리](assets/screenshot-library.png)

---

## 기능

- **라이브러리**: 로컬 폴더 스캔(하위 폴더 포함), 썸네일 그리드 탐색, 파일명 검색.
- **고정 배경화면**: 썸네일 또는 "배경화면으로 설정" 클릭 시 즉시 적용.
- **슬라이드쇼**: 지정 간격으로 자동 전환, 순차/랜덤, 다음 전환까지 남은 시간 진행률 표시.
- **표시 모드**: 채우기 / 맞춤 / 늘이기 / 가운데 / 타일 (Windows 는 레지스트리 사용).
- **테마**: 라이트 / 다크 즉시 전환.
- **설정 저장**: 폴더, 모드, 간격, 표시 모드, 테마, 마지막 배경화면 자동 저장.
- **단축키**: `←` 이전, `→` 다음.
- **단일 바이너리, 순수 Rust**: WebView/런타임 불필요, CJK 폰트 내장.

## 화면

| 라이브러리 | 슬라이드쇼 |
|---|---|
| ![라이브러리](assets/screenshot-library.png) | ![슬라이드쇼](assets/screenshot-slideshow.png) |

## 플랫폼

| 플랫폼 | 상태 | 배경화면 설정 |
|---|---|---|
| Windows x64 | ✅ 빌드 검증 | `SystemParametersInfoW` + 레지스트리 |
| macOS | ✅ 소스 지원 | `osascript`(System Events) |
| Linux | ✅ 소스 지원 | GNOME `gsettings` 또는 `feh` |

## 설정 파일

| 플랫폼 | 경로 |
|---|---|
| Windows | `%APPDATA%\wallpaper-rs\config.json` |
| macOS | `~/Library/Application Support/wallpaper-rs/config.json` |
| Linux | `$XDG_CONFIG_HOME/wallpaper-rs/config.json` (기본 `~/.config/wallpaper-rs/config.json`) |

## 빌드

Rust 1.75+ (stable 권장).

```bash
cargo build --release
```

## 라이선스

[MIT](LICENSE) © shuying-code
