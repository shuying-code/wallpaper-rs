# wallpaper-rs

<p align="center">🌐 <b>Language</b>：<a href="README.md">简体中文</a> · <a href="README.en.md">English</a> · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.pt-BR.md">Português</a></p>

**Rust + egui/eframe** 로 만든 **크로스 플랫폼 데스크톱 배경화면 앱**(Windows / macOS / Linux).
하나의 UI 로 **고정 배경화면**·**타이머 슬라이드쇼**·**라이브 배경화면**(동영상)을 지원합니다.
기본으로 **"온라인 라이브러리"** 를 엽니다. **로그인·구독·텔레메트리 없음.**

<p>
  <img alt="build" src="https://github.com/shuying-code/wallpaper-rs/actions/workflows/build.yml/badge.svg">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.75%2B-orange">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-2f6fed">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green">
</p>

![온라인 라이브러리](assets/screenshot-online.png)
![라이브러리](assets/screenshot-library.png)

---

## 기능

- **온라인 라이브러리(멀티 사이트, 기본 홈)**: Bing 일일 4K · Netbian · MoeWalls · MotionBGs 4개 사이트. 검색 / 카테고리 / 페이지 / 즐겨찾기를 지원하며, 원본 다운로드와 배경화면·잠금화면 설정이 가능합니다. 라이브 사이트는 MP4 다운로드와 **라이브 배경화면 설정**도 지원합니다.
- **라이브 배경화면(Windows / macOS / Linux-X11)**: 데스크톱 아이콘 뒤에 자체 호스트를 만들고 mpv 로 MP4 를 반복 재생합니다. **라이브 배경화면도 슬라이드쇼에 참여**할 수 있습니다. mpv 는 동봉하지 않으며, Windows 는 첫 사용 시 **여러 미러 속도를 측정해 가장 빠른 곳에서 자동 다운로드**하고 macOS / Linux 는 mpv 설치가 필요합니다.
- **라이브러리**: 로컬 폴더를 스캔(하위 폴더 포함)해 **이미지와 동영상**을 썸네일로 표시하고 파일명으로 검색. 동영상은 라이브 배경화면/잠금화면으로 설정하거나 슬라이드쇼에 추가할 수 있습니다.
- **고정 배경화면**: 썸네일 또는 "배경화면으로 설정" 클릭 시 즉시 적용.
- **슬라이드쇼**: 지정 간격으로 자동 전환, 순차/랜덤, 다음 전환까지 남은 시간 진행률 표시. **슬라이드쇼 모드면 실행 시 자동으로 시작하고, 여는 순간 한 장 먼저 바꿉니다.**
- **표시 모드**: 채우기 / 맞춤 / 늘이기 / 가운데 / 타일 (Windows 는 레지스트리 사용).
- **테마**: 라이트 / 다크 즉시 전환.
- **설정 저장**: 폴더, 모드, 간격, 표시 모드, 테마, 마지막 배경화면 자동 저장.
- **단축키**: `←` 이전, `→` 다음.
- **단일 바이너리, 순수 Rust**: WebView/런타임 불필요, CJK 폰트 내장.
- **로그인 시 자동 시작**：원클릭 토글. Windows 는 Run 레지스트리, macOS 는 LaunchAgent, Linux 는 autostart.

## 화면

| 라이브러리 | 슬라이드쇼 |
|---|---|
| ![라이브러리](assets/screenshot-library.png) | ![슬라이드쇼](assets/screenshot-slideshow.png) |

## 플랫폼

| 플랫폼 | 상태 | 배경화면 설정 |
|---|---|---|
| Windows x64 | ✅ 빌드 검증 | `SystemParametersInfoW` + 레지스트리; **라이브 배경화면**: 아이콘 뒤 자체 호스트 + mpv |
| macOS | ✅ 소스 지원 | `osascript`(System Events); **라이브 배경화면**: 데스크톱 레이어 창 + mpv |
| Linux | ✅ 소스 지원 | GNOME `gsettings` 또는 `feh`; **라이브 배경화면**: X11 `_NET_WM_WINDOW_TYPE_DESKTOP` + mpv(Wayland 미지원) |

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
