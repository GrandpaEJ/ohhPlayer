# ohhPlayer

An ultra-lightweight, ultra-fast video player written in **Rust**, powered by **Slint** (FemtoVG backend), **FFmpeg**, and **SDL2**.

Designed strictly to be tiny, highly responsive, and use minimal system resources without sacrificing modern media player features.

---

## Key Features

* 📺 **Picture-in-Picture (PiP) Mode**: Shrink into a floating top-pinned window with a single click or `P` key.
* 📱 **Fully Responsive UI**: Controls auto-adapt across window dimensions from small floating PiP boxes to 4K displays.
* 📐 **Flexible Video Scaling**: Cycle through **Fit (Letterbox)**, **Stretch**, **Zoom (Crop)**, **100% Native**, **1:1**, **16:9**, and **9:16** aspect ratios (`S` key).
* 🖱️ **Mouse Gestures**:
  * Double-click video to toggle Fullscreen.
  * Mouse wheel scroll over video to adjust Volume (±5% per tick).
* ⚡ **Ultra-Lean Resource Usage**: Single-demuxer thread model, bounded audio buffers, and hardware-accelerated FemtoVG rendering.
* 💾 **Automatic Position Resume**: Saves playback progress per file automatically.
* 📜 **Recent Files History**: Easily reopen past files via the integrated top menu overlay.
* ⏱️ **Sleep Timer & Speed Control**: 0.25x – 2.0x playback speed adjustments and custom auto-shutdown timers.
* ⌨️ **Rich Keyboard & Mouse Controls**: Comprehensive hotkey coverage for instant playback control.

---

## 🖥️ Picture-in-Picture (PiP) Across All DEs & WMs

When PiP mode is toggled (`P` key or PiP button), **ohhPlayer** automatically resizes to a compact size (`360×202`), forces `always_on_top`, and updates its window title to `"Picture-in-Picture"`.

### Desktop Environment & Window Manager Compatibility

* **Floating DEs (GNOME, KDE Plasma, XFCE, Cinnamon, MATE)**:
  Works out-of-the-box! Setting `always_on_top` keeps the PiP window floating on top of all workspace windows.

* **Tiling Window Managers (Niri, Hyprland, Sway, River, i3, bspwm)**:
  To allow the PiP window to automatically float and pin when PiP mode is activated, add the rule for your window manager:

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

## Keyboard & Mouse Shortcuts

| Key / Gesture | Action |
|---|---|
| `Space` / `K` | Play / Pause |
| `F` / Double-Click | Fullscreen Toggle |
| `P` | Picture-in-Picture (PiP) Mode |
| `S` | Cycle Video Scale Mode |
| `Left` / `Right` / `,` / `.` | Seek −5s / +5s |
| `J` / `L` | Seek −10s / +10s |
| `0` – `9` | Jump to 0% – 90% of duration |
| `↑` / `↓` / Mouse Scroll | Volume Up / Down (±5%) |
| `M` | Mute / Unmute |
| `?` / `H` | Show Keyboard Help |
| `Esc` / `Q` | Quit Player |

---

## Installation & Build (Linux)

### System Dependencies

```bash
sudo apt install libavcodec-dev libavformat-dev libswscale-dev libswresample-dev libsdl2-dev
```

### Build & Run

```bash
# Debug build
cargo run -- path/to/video.mp4

# Release build (optimized for max speed & minimal RAM)
cargo run --release -- path/to/video.mp4

# Install binary to PATH
cargo install --path .
ohhplayer path/to/video.mp4
```

---

## License

[MIT](LICENSE)
