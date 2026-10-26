# FastCam

**A tiny, fast webcam app for Windows.** Live mirrored preview, photos, and MP4 video with
microphone audio — written in Rust, drawn on the CPU, shipped as one ~1 MB `.exe` with no
runtime dependencies.

[![Latest release](https://img.shields.io/github/v/release/sayedmdsafwan/fastcam?label=download&color=2ea043)](https://github.com/sayedmdsafwan/fastcam/releases/latest)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%20%7C%2011-0078D6?logo=windows&logoColor=white)](#)
[![Rust](https://img.shields.io/badge/rust-2024%20edition-DEA584?logo=rust&logoColor=white)](https://www.rust-lang.org)
[![Size](https://img.shields.io/badge/exe-~1%20MB-blue)](#)

FastCam does one thing and does it quickly: open the camera, show the picture, take photos and
record video. No Electron, no GPU pipeline, no installer that wants 300 MB of disk.

---

## Features

| | |
|---|---|
| **Live preview** | Mirrored (like a mirror), always cropped to 16:9, scaled to the window with black bars when needed |
| **Photos** | `SPACE` → JPEG, quality 92, saved to `Pictures\FastCam` |
| **Video** | `V` → MP4 (H.264 video + AAC microphone audio) in `Videos\FastCam` |
| **Filters** | Normal, Bright, Gray, Warm — applied to the preview, the photo and the video identically |
| **Quality switch** | Best (the camera's own maximum) or Light 640×360, with `Q` |
| **Camera switch** | `C` cycles through every connected camera and remembers your choice |
| **Remembers state** | Filter, quality and camera survive a restart (`%APPDATA%\FastCam\settings.txt`) |
| **Never overwrites** | Every photo and video gets a unique file name |
| **Auto reconnect** | Camera unplugged or grabbed by another app? It keeps retrying; a running video is saved and stopped |
| **High-DPI aware** | Buttons and hint text scale with the Windows display scale |
| **No runtime needed** | Static CRT — no Visual C++ redistributable, works on a fresh Windows install |

## Download & install

Grab **`FastCam_Setup.exe`** from the [latest release](https://github.com/sayedmdsafwan/fastcam/releases/latest) and run it.

- Installs per-user, **no administrator rights** required
- Adds a **Desktop** shortcut and a **Start Menu** entry
- Uninstall from **Settings → Apps → FastCam**

> Windows Privacy settings must allow desktop apps to use the camera
> (**Settings → Privacy & security → Camera**) and the microphone
> (**Settings → Privacy & security → Microphone**).

## Keyboard shortcuts

| Key | Action |
|---|---|
| `SPACE` | Take a photo |
| `V` | Start / stop video |
| `M` | Microphone on / off while recording |
| `O` | Open the Photos folder |
| `SHIFT` + `O` | Open the Videos folder |
| `F11` | Full screen (`ESC` leaves it) |
| `C` | Next camera |
| `Q` | Best / light quality |
| `B` | Change filter |
| `H` or `F1` | Show / hide the shortcut panel |

The two round buttons at the bottom do the same as `SPACE` and `V`.

## Performance

Measured on a laptop with a 720p integrated webcam:

| | |
|---|---|
| Window on screen | **60–95 ms** after launch |
| First camera frame | **≈ 0.9–1.2 s** (device open ~150–300 ms + camera warm-up ~600 ms) |
| Preview while idle | CPU only, no GPU; one frame copy per displayed frame |

The camera is opened **once**; FastCam reads the camera's own format list and sets the best entry
from it. Asking a driver for a format it cannot deliver costs ~0.5 s per attempt, which is why many
camera apps feel slow to start.

## Build from source

Requirements: Rust stable (edition 2024) and Windows.

```cmd
git clone https://github.com/sayedmdsafwan/fastcam.git
cd fastcam
cargo build --release
```

The binary appears at `target\release\fastcam.exe`.

**Build the installer** — install [Inno Setup 6](https://jrsoftware.org/isinfo.php), then:

```cmd
"%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe" FastCam.iss
```

The result is `installer\FastCam_Setup.exe`.

## How it works

Everything lives in **[`src/main.rs`](src/main.rs)** — its header documents the whole design.

- **UI thread** — `winit` window, `softbuffer` pixels, keyboard and mouse. All drawing is plain
  CPU code: anti-aliased circles, a 5×7 pixel font, and a fast scaling loop for the preview.
- **Camera thread** — `nokhwa` over Media Foundation: grab a frame, crop to 16:9, mirror it, apply
  the filter, publish it for the UI and hand it to the recording worker.
- **Recording worker** — `windows` crate: an `IMFSinkWriter` writing H.264 into an MP4. A small
  queue drops frames rather than stalling the preview if the encoder falls behind.
- **Audio thread** — WASAPI capture converted to 48 kHz stereo 16-bit, written ~150 ms behind a
  shared clock so audio and video stay in sync.

Frames are RGB (`w * h * 3`); the video writer converts them to RGB32 bottom-up, which Media
Foundation wants.

## Where files go

| Path | Contents |
|---|---|
| `%USERPROFILE%\Pictures\FastCam` | `photo_<unixtime>.jpg` |
| `%USERPROFILE%\Videos\FastCam` | `video_<unixtime>.mp4` |
| `%APPDATA%\FastCam\settings.txt` | `filter=`, `light=`, `camera=` — delete it to reset |

## Troubleshooting

| Symptom | Fix |
|---|---|
| Black preview, title says "camera open failed" | Windows camera privacy is off, or another app holds the camera |
| "mic blocked" while recording | Allow desktop apps to use the microphone in Windows privacy settings |
| Choppy preview, low frame rate | Usually low light — the camera lowers its frame rate to keep the picture bright. Check the room lighting, or press `Q` for 360p |
| "camera picture cannot be decoded" | Press `Q` (some cameras ship broken MJPEG at high resolution) |
| Video will not open | The app was force-killed mid-recording; close the app normally so the MP4 gets finalized |
| Switching camera/quality refused | On purpose while recording — stop the recording first |

## Project layout

```
src/main.rs          the entire application
Cargo.toml           dependencies and release profile (LTO, strip, panic=abort)
Cargo.lock           pinned dependency versions
build.rs             embeds icon.ico into the exe
icon.ico             application icon
.cargo/config.toml   static CRT so the exe has no VC++ runtime dependency
FastCam.iss          Inno Setup installer script
```

## Contributing

Issues and pull requests are welcome. Please keep changes small and focused: FastCam values speed
and a dependency list you can read.
