# AGENTS.md — ohhPlayer Contributor & AI Agent Rules

> This file defines the rules, architecture, and conventions that **all contributors and AI agents** must follow when working on ohhPlayer. Read this before touching any code.

---

## Project Overview

**ohhPlayer** is a minimal, production-quality video player written in Rust. It is strictly optimized to be tiny, fast, and use as few resources as possible.

| Layer | Technology | Purpose |
|---|---|---|
| UI | [Slint](https://slint.dev) (`.slint` files) | Declarative UI, controls, animations (uses FemtoVG) |
| Video decode | FFmpeg (`ffmpeg-sys-next`) | Decode video packets, scale to RGB |
| Audio decode | FFmpeg + SDL2 | Decode audio packets, resample to 44100 Hz f32 |
| Audio playback | SDL2 (`sdl2` crate) | Push-mode audio callback |
| Glue | Rust `main.rs` | Wires UI callbacks ↔ demux/decoder threads |

---

## Architecture (Single Demuxer Model)

```
┌──────────────┐    Arc<Mutex<DecoderCommand>>    ┌─────────────────────┐
│   main.rs    │ ───────────────────────────────► │   Demux Thread      │
│  (Slint UI   │                                  │ (av_read_frame &    │
│   thread)    │ ◄─────────────────────────────── │  av_seek_frame)     │
│              │    Arc<Mutex<DecoderState>>      └───────┬──────┬──────┘
│              │                                          │      │
│              │    Arc<Mutex<Option<DecodedFrame>>>      │      │ video packet channel
│              │ ◄────────────────────────────────────────│◄─────┘
│              │                                          │
│              │    Arc<Mutex<AudioShared>>               │ audio packet channel
│              │ ────────────────────────────────────────►│
│   16ms timer │                                          ▼
│   (60 fps)   │                                  ┌─────────────────────┐
└──────────────┘                                  │   Decode Threads    │
                                                  │   (video & audio)   │
                                                  └──────────┬──────────┘
                                                             │ AudioShared.buffer
                                                  ┌──────────▼──────────┐
                                                  │  SDL AudioCallback  │
                                                  │  (audio hw thread)  │
                                                  └─────────────────────┘
```

### Key Shared State

| Type | Owner | Consumers | Purpose |
|---|---|---|---|
| `Arc<Mutex<DecoderCommand>>` | `main.rs` | Demux thread | Seek target, play/pause, quit, load_file, speed |
| `Arc<Mutex<DecoderState>>` | Demux / Video | `main.rs` timer | Current PTS position, duration, playing |
| `Arc<Mutex<Option<DecodedFrame>>>` | Video decode | `main.rs` timer | Latest RGB frame (overwritten each frame) |
| `Arc<Mutex<AudioShared>>` | `main.rs` | Audio decode + SDL | Volume, playing, buffer, seek_to |
| Channels (`crossbeam` / `mpsc`) | Demux thread | Decode threads | Bounded packet transmission (av_packet) |

---

## Thread Model

| Thread | Who spawns it | What it does |
|---|---|---|
| **Slint UI thread** | OS (main) | Runs the event loop, 16ms timer, all callbacks |
| **Demux thread** | `Decoder::start()` | Opens file ONE TIME, reads packets, pushes to channels. Handles seeking. |
| **Video decode thread** | `Decoder::start()` | Pulls video packets from channel → decodes → scales → writes `DecodedFrame` |
| **Audio decode thread** | `AudioOutput::start()`| Pulls audio packets from channel → decodes → resamples → pushes to `AudioShared.buffer` |
| **SDL audio hw thread** | SDL2 | Pulls from `AudioShared.buffer` → applies volume → outputs to hardware |

> ⚠️ **Never call Slint APIs from non-UI threads.** Use `app_weak.upgrade()` only inside the Slint timer callback or callbacks registered with `app.on_*()`.

---

## Coding Rules

### Memory & Performance Rules (CRITICAL)

1. **SINGLE DEMUXER ONLY.** Never call `avformat_open_input` in multiple threads for the same file. MP4 `moov` atoms (indexes) scale linearly with video duration and can easily consume 50-100MB per file handle. Doubling this by opening the file twice causes massive RAM bloat.
2. **Limit FFmpeg Threads.** Always set `codec_ctx.thread_count = 1`. Allowing FFmpeg to spin up threads per CPU core causes unnecessary memory overhead.
3. **Limit Probe Buffering.** Always pass `probesize=32000` and `analyzeduration=0` to `avformat_open_input` to stop FFmpeg from over-buffering megabytes of packets just to find stream info.
4. **Bounded Channels & Backpressure.** Video and audio packet channels must be tightly bounded (e.g., 10-20 packets) so the demux thread sleeps when the decoders fall behind. The audio PCM buffer must be strictly capped at `2 seconds` (`44100 * 2 * 2`).
5. **UI Renderer:** Always default to `winit-femtovg` backend for Slint, as it avoids Skia's massive RAM consumption on X11 environments.

### General & Synchronization

6. **No `unwrap()` on locks in the SDL callback.** The SDL callback runs on a real-time audio thread. Use `.try_lock()` with a fallback to silence.
7. **Lock Ordering.** Never hold two locks simultaneously without a strict lock-ordering convention (audio_shared < decoder_cmd < decoder_state) to prevent deadlock.
8. **Frame overwrite is intentional.** `frame_out` holds only the latest decoded frame. The UI timer takes it with `.take()`.
9. **All UI state mutations must happen on the Slint thread.** Use `slint::invoke_from_event_loop` if needed.

### Slint Rules

10. **Do not use `.clamp()` in Slint.** It is a Rust method. Use `max(0, min(1, expr))` instead.
11. **Do not use `float == float` for speed comparisons in Slint.** Floating-point equality is unreliable.
12. **Keep `.slint` files in `ui/`.** Never generate Slint code from Rust strings at runtime.
13. **`block-slider-update`** must be set to `true` before programmatically changing `slider-value`, then immediately set back to `false`.
14. **`pointer-event-transparent` does not exist on `TouchArea`** in Slint 1.x. Use `enabled: false`.
15. **`has_hover` uses an underscore, not a hyphen.** 
16. **`TouchArea.moved` only fires while a mouse button is pressed (drag).** To detect free mouse movement, use `pointer-event(evt) => { if evt.kind == PointerEventKind.move { ... } }`.

### Decoder & Seek Rules

17. **Seek Workflow:** Demux thread receives seek target -> flushes packet channels -> calls `av_seek_frame` -> sends "Flush" signal packet to decoder threads. Decoder threads call `avcodec_flush_buffers` when they receive the flush signal.
18. **Frame-pacing anchor must be reset on every seek.**
19. **Skip frames with invalid PTS** (`i64::MIN` or `i64::MAX`). 
20. **Speed scaling:** divide `pts_elapsed` by `speed` before comparing to `real_elapsed`.

### Audio Rules

21. **On pause: SDL callback outputs silence.** Do not drain the PCM buffer while paused.
22. **Volume is applied in the SDL callback**, not at encode time. Raw f32 samples in `buffer` are always `[-1.0, 1.0]`.
23. **Audio seek uses `audio_tb` (audio stream time base).**

---

## Known Limitations

| Issue | Why not fixed | Notes |
|---|---|---|
| No A/V sync master clock | Requires audio-clock-driven video sync (PTS drift correction) | Current approach: video paces itself via PTS; audio is eventually consistent after seek |
| Speed > 1× audio pitch | `atempo` FFmpeg filter required | SDL doesn't support pitch-shifting; speed only affects video frame pacing |
| No subtitle support | Out of scope | Would need a separate subtitle demux + render layer |
| Single file only | No playlist/queue | `args[1]` is the only input |

---

## Build & Run

```bash
# Debug build
cargo build

# Release (optimized for size & speed)
cargo build --release
./target/release/ohhplayer path/to/video.mp4
```

**Required system dependencies:**
- `libavcodec`, `libavformat`, `libswscale`, `libswresample` (FFmpeg ≥ 6)
- `libsdl2-dev`

---

## File Structure

```
ohhPlayer/
├── src/
│   ├── main.rs        # Entry point, Slint callbacks, UI timer
│   ├── decoder/       # Demuxer, video decode, channels, frame-pacing
│   ├── audio/         # Audio decode thread, SDL playback, AudioShared
│   └── ui_state.rs    # Opacity/animation helpers, time formatting
├── ui/
│   ├── appwindow.slint  # Root window, property routing
│   └── ...
├── build.rs           # Slint build script
├── Cargo.toml
└── AGENTS.md          # ← You are here
```

---

## Commit Convention

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add single demuxer thread
fix: reset frame-pacing anchor on seek
refactor: consolidate audio state into AudioShared
perf: reduce audio buffer cap from 10s to 5s
```
