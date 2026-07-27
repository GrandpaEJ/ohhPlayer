# ohhPlayer

[![Release](https://img.shields.io/github/v/release/GrandpaEJ/ohhPlayer?color=8a2be2&style=flat-square)](https://github.com/GrandpaEJ/ohhPlayer/releases)
[![License](https://img.shields.io/badge/license-MIT-blue.svg?style=flat-square)](LICENSE)
[![Rust](https://img.shields.io/badge/built%20with-Rust-orange.svg?style=flat-square)](https://www.rust-lang.org/)

An ultra-lightweight, ultra-fast video player written in **Rust**, powered by **Slint** (FemtoVG backend), **FFmpeg**, and **SDL2**.

Designed strictly to be tiny, highly responsive, and use minimal system resources without sacrificing modern media player capabilities.

---

## 🌟 Key Features

* 📺 **Universal Picture-in-Picture (PiP)**: Shrink into a floating top-pinned window with a single click or `P` key (includes native IPC auto-floating for Niri, Hyprland, Sway, and i3).
* 🎵 **Real-time Audio Delay Adjustment**: Shift audio timing on-the-fly (`Z` / `X` for ±50ms sync shift, `C` to reset) to fix A/V desync in videos.
* ⚡ **Progressive Long-Hold Fast Seeking**: Press or hold down seek keys (`Left`/`Right`, `,`/`.`, `J`/`L`) or on-screen buttons to smoothly accelerate seek steps from 1s ➔ 2s ➔ 5s ➔ 10s ➔ 15s.
* 🎞️ **High Quality Decoder Engine**: Features `Bicubic` + `Accurate_RND` high-fidelity video frame scaling and 32-tap linear audio resampling.
* 🎛️ **Comprehensive On-Screen Controls**: Dedicated `-10s`, `-5s`, `+5s`, and `+10s` skip control buttons alongside volume, speed, scale, and PiP controls.
* 📐 **Flexible Video Scale Modes**: Cycle through **Fit (Letterbox)**, **Stretch**, **Zoom (Crop)**, **100% Native**, **1:1**, **16:9**, and **9:16** aspect ratios (`S` key).
* 🖱️ **Mouse Gestures & Shortcuts**:
  * Double-click video to toggle Fullscreen.
  * Mouse wheel scroll over video to adjust Volume (±5% per tick).
* 💾 **Automatic Resume & History**: Saves playback position per file automatically and tracks recent files in the top menu overlay.
* ⏱️ **Sleep Timer & Speed Control**: 0.25x – 2.0x playback speed adjustments and custom auto-shutdown timers.

---

## 🖥️ Picture-in-Picture (PiP) Across All DEs & WMs

When PiP mode is toggled (`P` key or PiP button), **ohhPlayer** automatically resizes to a compact size (`360×202`), forces `always_on_top`, and updates its window title to `"Picture-in-Picture"`.

### Desktop Environment & Window Manager Compatibility

* **Floating DEs (GNOME, KDE Plasma, XFCE, Cinnamon, MATE)**:
  Works out-of-the-box! Setting `always_on_top` keeps the PiP window floating on top of all workspace windows.

* **Tiling Window Managers (Niri, Hyprland, Sway, River, i3, bspwm)**:
  Automatic IPC commands trigger floating state upon PiP toggle. To ensure consistent dimensions, add the rule for your window manager:

#### 1. Niri (`~/.config/niri/config.kdl`)
```kdl
window-rule {
    match title="Picture-in-Picture"
    open-floating true
    default-column-width { fixed 360; }
    default-window-height { fixed 202; }
}
```

#### 2. Hyprland (`~/.config/hypr/hyprland.conf`)
```ini
windowrulev2 = float, title:^(Picture-in-Picture)$
windowrulev2 = pin, title:^(Picture-in-Picture)$
windowrulev2 = size 360 202, title:^(Picture-in-Picture)$
```

#### 3. Sway / i3 (`~/.config/sway/config` or `~/.config/i3/config`)
```ini
for_window [title="Picture-in-Picture"] floating enable, sticky enable, resize set 360 202
```

---

## ⌨️ Keyboard & Mouse Shortcuts

| Key / Gesture | Action |
|---|---|
| `Space` / `K` | Play / Pause |
| `F` / Double-Click | Fullscreen Toggle |
| `P` | Picture-in-Picture (PiP) Mode |
| `S` | Cycle Video Scale Mode |
| `Left` (←) / `Right` (→) / `,` / `.` | Seek −5s / +5s (Progressive acceleration on hold) |
| `J` / `L` | Seek −10s / +10s |
| `0` – `9` | Jump to 0% – 90% of duration |
| `Z` / `X` | Audio Delay −50ms / +50ms (Sync Shift) |
| `C` | Reset Audio Delay (0ms) |
| `↑` / `↓` / Mouse Scroll | Volume Up / Down (±5%) |
| `M` | Mute / Unmute |
| `?` / `H` | Show Keyboard Help |
| `Esc` / `Q` | Quit Player |

---

## 🛠️ Installation & Build (Linux)

### System Dependencies

```bash
sudo apt install libavcodec-dev libavformat-dev libswscale-dev libswresample-dev libsdl2-dev
```

### 📦 Easy 1-Click Install Script

```bash
./install.sh
```

### 🚀 Build Portable AppImage (5.7 MB)

```bash
./scripts/build_appimage.sh
# Output binary: target/ohhPlayer-x86_64.AppImage
```

### Manual Build & Run

```bash
# Debug build
cargo run -- path/to/video.mp4

# Release build (optimized for max speed & minimal RAM)
cargo run --release -- path/to/video.mp4
```

---

## 📄 License

[MIT](LICENSE)
