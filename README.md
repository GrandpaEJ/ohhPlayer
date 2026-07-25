# ohhPlayer

Just a video player with the features I want.

## Features

- Play/pause, seek, volume, speed control
- Fullscreen, multiple scale modes (Fit, Stretch, Zoom, 1:1, 16:9, etc.)
- Keyboard shortcuts for everything
- Automatic resume — saves position per file
- Recent files history
- Sleep timer
- On-screen display

## Usage

```bash
# Build & run
cargo run --release -- path/to/video.mp4

# Or install and run from anywhere
cargo install --path .
ohhplayer path/to/video.mp4
```

### Keys

| Key | Action |
|---|---|
| `Space` | Play / Pause |
| `f` | Fullscreen |
| `j` / `,` | Seek −10 / −5s |
| `l` / `.` | Seek +10 / +5s |
| `↑` / `↓` | Volume ±5% |
| `m` | Mute |
| `?` / `h` | Keyboard help |
| `Esc` / `q` | Quit |

## Install (Linux)

```bash
sudo apt install libavcodec-dev libavformat-dev libswscale-dev libswresample-dev libsdl2-dev
cargo install --path .
```

## License

MIT
