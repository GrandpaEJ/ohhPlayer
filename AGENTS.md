# AGENTS.md — ohhPlayer Contributor & AI Agent Rules

> This file defines the rules, architecture, and conventions that **all contributors and AI agents** must follow when working on ohhPlayer. Read this before touching any code.

---

## Project Overview

**ohhPlayer** is a minimal, production-quality video player written in Rust.

| Layer | Technology | Purpose |
|---|---|---|
| UI | [Slint](https://slint.dev) (`.slint` files) | Declarative UI, controls, animations |
| Video decode | FFmpeg (`ffmpeg-sys-next`) | Demux, decode, scale to RGB |
| Audio decode | FFmpeg + SDL2 | Demux, decode, resample to 44100 Hz f32 |
| Audio playback | SDL2 (`sdl2` crate) | Push-mode audio callback |
| Glue | Rust `main.rs` | Wires UI callbacks ↔ decoder/audio threads |

---

## Architecture

```
┌──────────────┐     Arc<Mutex<DecoderCommand>>     ┌─────────────────────┐
│   main.rs    │ ──────────────────────────────────► │  decoder thread     │
│  (Slint UI   │                                     │  (decode_video)     │
│   thread)    │ ◄────────────────────────────────── │                     │
│              │     Arc<Mutex<DecoderState>>         └─────────────────────┘
│              │     Arc<Mutex<Option<DecodedFrame>>>
│              │
│              │     Arc<Mutex<AudioShared>>          ┌─────────────────────┐
│              │ ──────────────────────────────────► │  audio decode thread │
│              │                                     │  (decode_audio)      │
│              │                                     └──────────┬──────────┘
│              │                                                │ AudioShared.buffer
│   16ms timer │                                     ┌──────────▼──────────┐
│   (60 fps)   │                                     │  SDL AudioCallback   │
└──────────────┘                                     │  (audio hw thread)   │
                                                     └─────────────────────┘
```

### Key Shared State

| Type | Owner | Consumers | Purpose |
|---|---|---|---|
| `Arc<Mutex<DecoderCommand>>` | `main.rs` | decoder thread | Seek target, play/pause, quit, speed |
| `Arc<Mutex<DecoderState>>` | decoder thread | `main.rs` timer | Current PTS position, duration, playing |
| `Arc<Mutex<Option<DecodedFrame>>>` | decoder thread | `main.rs` timer | Latest RGB frame (overwritten each frame) |
| `Arc<Mutex<AudioShared>>` | `main.rs` | audio decode + SDL | Volume, playing, buffer, seek_to |

---

## Thread Model

| Thread | Who spawns it | What it does |
|---|---|---|
| **Slint UI thread** | OS (main) | Runs the event loop, 16ms timer, all callbacks |
| **Video decode thread** | `Decoder::start()` | Reads video packets → decodes → scales → writes `DecodedFrame` |
| **Audio decode thread** | `AudioOutput::start()` | Reads audio packets → decodes → resamples → pushes to `AudioShared.buffer` |
| **SDL audio hw thread** | SDL2 | Pulls from `AudioShared.buffer` → applies volume → outputs to hardware |

> ⚠️ **Never call Slint APIs from non-UI threads.** Use `app_weak.upgrade()` only inside the Slint timer callback or callbacks registered with `app.on_*()`.

---

## Coding Rules

### General

1. **No `unwrap()` on locks in the SDL callback.** The SDL callback runs on a real-time audio thread. A poisoned lock will silently corrupt audio. Use `.try_lock()` with a fallback to silence.
2. **Never hold two locks simultaneously** without a strict lock-ordering convention (audio_shared < decoder_cmd < decoder_state) to prevent deadlock.
3. **Frame overwrite is intentional.** `frame_out` holds only the latest decoded frame. The UI timer takes it with `.take()`. Do not turn this into a queue without adding back-pressure.
4. **Audio buffer back-pressure is capped at 5 seconds.** The audio decode thread sleeps when `buffer.len() >= 44100 * 2 * 5`. Do not raise this limit; it inflates RAM and worsens seek latency.
5. **All UI state mutations must happen on the Slint thread.** Use `slint::invoke_from_event_loop` if you ever need to update UI from another thread.

### Slint Rules

6. **Do not use `.clamp()` in Slint.** It is a Rust method, not a Slint builtin. Use `max(0, min(1, expr))` instead.
7. **Do not use `float == float` for speed comparisons in Slint.** Floating-point equality is unreliable. Use integer-keyed speed variants or an epsilon check if comparing dynamically set values.
8. **Keep `.slint` files in `ui/`.** Never generate Slint code from Rust strings at runtime.
9. **The `Controls` component owns no mutable state** except `speed-open` (popup visibility). All other state flows down as `in-out property` from `AppWindow`.
10. **`block-slider-update`** must be set to `true` before programmatically changing `slider-value`, then immediately set back to `false`. Failing to do so triggers a seek feedback loop.

### Decoder Rules

11. **Frame-pacing anchor (`wall_start`, `pts_start`) must be reset on every seek.** Forgetting this causes the decoder to sleep for minutes after a backward seek.
12. **`pause_elapsed` must accumulate paused time** so that frame-pacing doesn't drift after a pause/resume cycle.
13. **`seek_target` must be checked at the top of every packet loop iteration**, not only for video packets. Non-video packet branches previously caused seek commands to be silently dropped.
14. **Skip frames with invalid PTS** (`i64::MIN` or `i64::MAX`). Do not attempt to use them for frame-pacing or position tracking.
15. **Speed scaling:** divide `pts_elapsed` by `speed` before comparing to `real_elapsed`. Speed = 0.0 is handled as 1.0 to avoid division-by-zero.

### Audio Rules

16. **On seek: flush codec buffers AND clear `AudioShared.buffer`.** Leaving stale samples in the buffer causes permanent A/V desync after seeking.
17. **On pause: SDL callback outputs silence.** The decode thread also yields. Do not drain the buffer while paused; resuming would restart from the wrong position.
18. **Volume is applied in the SDL callback**, not at encode time. Raw f32 samples in `buffer` are always `[-1.0, 1.0]` (pre-volume).
19. **Audio seek uses `audio_tb` (audio stream time base)**, not the video stream time base. Always use the correct time base for the stream being seeked.

---

## Known Limitations (Do Not "Fix" Without Reading This)

| Issue | Why not fixed | Notes |
|---|---|---|
| No A/V sync master clock | Requires audio-clock-driven video sync (PTS drift correction) | Current approach: video paces itself via PTS; audio is eventually consistent after seek |
| Speed > 1× audio pitch | `atempo` FFmpeg filter required | SDL doesn't support pitch-shifting; speed only affects video frame pacing |
| No subtitle support | Out of scope | Would need a separate subtitle demux + render layer |
| Window is decorated | OS title bar provides close/min/max | Do NOT re-add those buttons to the controls bar |
| Single file only | No playlist/queue | `args[1]` is the only input |

---

## Build & Run

```bash
# Debug build
cargo build

# Run with a video file
cargo run -- path/to/video.mp4

# Release (optimized)
cargo build --release
./target/release/ohhplayer path/to/video.mp4
```

**Required system dependencies:**
- `libavcodec`, `libavformat`, `libswscale`, `libswresample` (FFmpeg ≥ 6)
- `libsdl2-dev`

---

## Keyboard Shortcuts

| Key | Action |
|---|---|
| `Space` | Play / Pause |
| `f` / `F` | Toggle fullscreen |
| `j` | Seek −10 s |
| `l` | Seek +10 s |
| `,` | Seek −5 s |
| `.` | Seek +5 s |

---

## File Structure

```
ohhPlayer/
├── src/
│   ├── main.rs        # Entry point, Slint callbacks, UI timer
│   ├── decoder.rs     # Video decode thread, frame-pacing, seek
│   ├── audio.rs       # Audio decode thread, SDL playback, AudioShared
│   └── ui_state.rs    # Opacity/animation helpers, time formatting
├── ui/
│   ├── appwindow.slint  # Root window, property routing
│   ├── controls.slint   # Bottom controls bar (seek, volume, speed, fullscreen)
│   └── center-button.slint  # Center play/pause overlay button
├── build.rs           # Slint build script
├── Cargo.toml
└── AGENTS.md          # ← You are here
```

---

## Commit Convention

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add subtitle support
fix: reset frame-pacing anchor on seek
refactor: consolidate audio state into AudioShared
perf: reduce audio buffer cap from 10s to 5s
docs: update AGENTS.md with audio rules
```

Never commit a build that fails `cargo build` or has `#[allow(unused)]` masking real bugs.
