# wallpaper-rs

<p align="center">🌐 <b>Language</b>：<a href="README.md">简体中文</a> · <a href="README.en.md">English</a> · <a href="README.zh-TW.md">繁體中文</a> · <a href="README.ja.md">日本語</a> · <a href="README.ko.md">한국어</a> · <a href="README.ru.md">Русский</a> · <a href="README.es.md">Español</a> · <a href="README.fr.md">Français</a> · <a href="README.de.md">Deutsch</a> · <a href="README.pt-BR.md">Português</a></p>

Une **application de fond d'écran multiplateforme** écrite en **Rust + egui/eframe**
(Windows / macOS / Linux). Une seule interface gère le **fond fixe**, le **diaporama
minuté** et les **fonds animés** (vidéo). Elle s'ouvre par défaut sur la
**Bibliothèque en ligne** ; **aucun compte, aucun abonnement, aucune télémétrie.**

<p>
  <img alt="build" src="https://github.com/shuying-code/wallpaper-rs/actions/workflows/build.yml/badge.svg">
  <img alt="Rust" src="https://img.shields.io/badge/rust-1.75%2B-orange">
  <img alt="Platform" src="https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-2f6fed">
  <img alt="License" src="https://img.shields.io/badge/license-MIT-green">
</p>

![Bibliothèque en ligne](assets/screenshot-online.png)
![Bibliothèque](assets/screenshot-library.png)

---

## Fonctionnalités

- **Bibliothèque en ligne (multi-sites, écran d'accueil)** : Bing du jour 4K · Netbian · MoeWalls · MotionBGs — quatre sites. Recherche / catégories / pagination / favoris ; téléchargement de l'original, fond d'écran ou écran de verrouillage. Les sites vidéo permettent aussi de télécharger le MP4 ou de **définir un fond animé**.
- **Fonds animés (Windows / macOS / Linux-X11)** : un hôte dédié **derrière les icônes du bureau**, mpv lisant le MP4 en boucle. **Les fonds animés participent aussi au diaporama.** mpv n'est pas fourni — sous Windows, au premier lancement il **mesure la vitesse de plusieurs miroirs et télécharge le plus rapide** ; sous macOS / Linux, mpv doit être installé.
- **Bibliothèque** : scanne un dossier local (sous-dossiers inclus) et affiche **images et vidéos** en grille avec recherche par nom. Les vidéos peuvent être définies en fond animé / écran de verrouillage ou ajoutées au diaporama.
- **Fond fixe** : cliquez sur une vignette ou « Définir comme fond » pour appliquer aussitôt.
- **Diaporama** : changement automatique par intervalle, séquentiel ou aléatoire, avec barre de progression. **En mode diaporama, il démarre seul à l'ouverture et change une image immédiatement.**
- **Mode d'affichage** : Remplir / Ajuster / Étirer / Centrer / Mosaïque (Windows via le registre).
- **Thème** : clair / sombre, application immédiate.
- **Config persistante** : dossier, mode, intervalle, affichage, thème et dernier fond.
- **Raccourcis** : `←` précédent, `→` suivant.
- **Binaire unique, Rust pur** : pas de WebView ni runtime, police CJK intégrée.
- **Lancement à la connexion** : un interrupteur — registre Run (Windows), LaunchAgent (macOS), autostart (Linux).

## Écrans

| Bibliothèque | Diaporama |
|---|---|
| ![Bibliothèque](assets/screenshot-library.png) | ![Diaporama](assets/screenshot-slideshow.png) |

## Plateformes

| Plateforme | État | Application du fond |
|---|---|---|
| Windows x64 | ✅ Compilé et vérifié | `SystemParametersInfoW` + registre ; **fond animé** : hôte derrière les icônes + mpv |
| macOS | ✅ Support code | `osascript` (System Events) ; **fond animé** : fenêtre au niveau du bureau + mpv |
| Linux | ✅ Support code | GNOME `gsettings` ou `feh` ; **fond animé** : X11 `_NET_WM_WINDOW_TYPE_DESKTOP` + mpv (Wayland non pris en charge) |

## Fichier de configuration

| Plateforme | Chemin |
|---|---|
| Windows | `%APPDATA%\wallpaper-rs\config.json` |
| macOS | `~/Library/Application Support/wallpaper-rs/config.json` |
| Linux | `$XDG_CONFIG_HOME/wallpaper-rs/config.json` (par défaut `~/.config/wallpaper-rs/config.json`) |

## Compilation

Rust 1.75+ requis (stable recommandé).

```bash
cargo build --release
```

## Licence

[MIT](LICENSE) © shuying-code
