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
