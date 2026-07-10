# ohhPlayer 🎬

<div align="center">
  <img src="https://img.shields.io/badge/Language-Rust-orange?style=flat-square&logo=rust" alt="Rust">
  <img src="https://img.shields.io/badge/UI-Slint-blue?style=flat-square" alt="Slint UI">
  <img src="https://img.shields.io/badge/Media-FFmpeg-green?style=flat-square&logo=ffmpeg" alt="FFmpeg">
  <img src="https://img.shields.io/badge/Audio-SDL2-lightblue?style=flat-square&logo=libsdl" alt="SDL2">
</div>

<br>

**ohhPlayer** is a minimal, blazing-fast, and production-quality video player written entirely in Rust. 

It leverages the power of **FFmpeg** for robust hardware/software media decoding, **SDL2** for low-latency audio playback, and **Slint** for a beautiful, declarative, and modern user interface.

## ✨ Features & Optimizations

We built ohhPlayer with a strict focus on resource efficiency. It is designed to run smoothly on low-end hardware and lightweight window managers (e.g., Niri, XFCE).

- **Ultra-Low Memory Footprint:** Idles at ~23MB RAM. During 1080p playback, it averages ~50-80MB. This is achieved by forcing the `winit-femtovg` renderer (bypassing the memory-heavy Skia backend) and aggressively capping audio PCM buffers.
- **Zero-Latency Startup:** Bypasses FFmpeg's default stream probe buffering by enforcing `probesize=32000` and `analyzeduration=0`. Videos open instantly without reading megabytes of metadata.
- **Zero-Asset UI:** The entire interface is drawn using raw mathematical SVG paths rendered directly by Slint. There are no external font dependencies, PNGs, or emoji glyphs taking up binary space or memory.
- **Smart Hardware Threading:** Limits FFmpeg's internal codec context threads to `1`, preventing unnecessary CPU core spin-ups and avoiding massive RAM bloat for MP4 `moov` atom parsing.
- **Graceful EOF Handling:** Independent audio and video thread EOF handling ensures that slightly mismatched A/V tracks don't cause frame stuttering or frozen master clocks at the end of a video.
- **Modern Glassmorphic UI:** Standardized 40x40px control pills, animated OSD overlays, smooth hover expansions, and dynamic scaling.
- **Smart Resume & History:** Automatically saves positions to `~/.ohhplayer_settings`. Seamlessly jump back into the last 15 videos via the hamburger menu.
- **Dynamic Scale Modes:** Switch effortlessly between Fit, Stretch, Zoom, 100%, 1:1, 16:9, and 9:16.

## 🏗 Architecture

ohhPlayer uses a heavily decoupled, multi-threaded architecture to ensure the UI timer (running at 16ms / 60 FPS) never blocks on FFmpeg I/O or decoding.

| Layer | Technology | Purpose |
|---|---|---|
| **UI** | [Slint](https://slint.dev) (`.slint` files) | Declarative UI, controls, animations, overlays |
| **Video Decode** | FFmpeg (`ffmpeg-sys-next`) | Independent demux/decode loop, frame-pacing, YUV to RGB scaling |
| **Audio Decode** | FFmpeg + SDL2 | Independent demux/decode loop, resampling to 44100 Hz f32 |
| **Audio Playback** | SDL2 (`sdl2` crate) | Real-time push-mode hardware callback |
| **Glue / State** | Rust `main.rs` & `app.rs` | Wires UI timer callbacks ↔ decoder/audio threads |

```mermaid
graph TD;
    UI[Slint UI Timer 60fps] -->|Mutex<DecoderCommand>| V_DEC[Video Thread av_read_frame]
    UI -->|Mutex<AudioShared>| A_DEC[Audio Thread av_read_frame]
    V_DEC -->|Mutex<Option<DecodedFrame>>| UI
    A_DEC -->|Bounded PCM Array| SDL[SDL Real-time HW Thread]
```

## ⌨️ Keyboard Shortcuts

| Key | Action |
|---|---|
| `Space` | Play / Pause |
| `f` / `F` | Toggle fullscreen |
| `j` | Seek −10 seconds |
| `l` | Seek +10 seconds |
| `,` | Seek −5 seconds |
| `.` | Seek +5 seconds |
| `Up` / `Down` | Adjust Volume ±5% |
| `m` / `M` | Mute / Unmute |
| `?` / `h` / `H` | Toggle Keyboard Shortcuts Help |
| `Escape` / `q`| Close Player |

## 🚀 Getting Started

### Prerequisites

Ensure you have the required system dependencies installed on your Linux machine:

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install libavcodec-dev libavformat-dev libswscale-dev libswresample-dev libsdl2-dev clang pkg-config
```

### Build & Run

```bash
# Clone the repository
git clone https://github.com/GrandpaEJ/ohhPlayer.git
cd ohhPlayer

# Build and run the app (without a file)
cargo run --release

# Run with a specific video file
cargo run --release -- path/to/video.mp4
```

*Note: For the best performance and frame-pacing, it is highly recommended to run ohhPlayer in `--release` mode.*

## 📂 File Structure

```text
ohhPlayer/
├── src/
│   ├── main.rs        # Entry point and initialization
│   ├── app.rs         # Slint callbacks, timers, window scaling, settings logic
│   ├── decoder/       # Video decoding thread, frame-pacing, seek handling
│   ├── audio/         # Audio decoding thread, SDL playback loop
│   ├── settings.rs    # Persistent JSON settings & play history manager
│   └── ui_state.rs    # Opacity/animation helpers, time formatting
├── ui/
│   ├── appwindow.slint      # Root window, property routing, overlays
│   ├── controls.slint       # Bottom controls bar (seek, volume, speed)
│   ├── top-menu.slint       # Hamburger menu & dropdown
│   ├── recent-files.slint   # Recent files list UI
│   ├── osd.slint            # On-Screen Display overlay
│   └── keyboard-help.slint  # Keyboard shortcuts popup
├── build.rs           # Slint build script
└── Cargo.toml         # Rust dependencies
```

## 📜 Contributing

If you'd like to contribute, please read the [AGENTS.md](AGENTS.md) file first. It acts as the source of truth for our architecture, threading rules, lock management conventions, and Slint design limitations.

Use **Conventional Commits** for all pull requests.

## 📄 License

This project is licensed under the MIT License.
