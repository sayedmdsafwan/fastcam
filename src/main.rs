// ============================================================================
// FastCam - a tiny, fast Windows webcam app (photo + video + mic) in Rust.
// This is the ONLY source file (src/main.rs). Read this header first.
// ============================================================================
//
// WHO / HOW THE OWNER WORKS (important for any AI reading this)
//   - Owner: Safwan. He is a "vibe coder": he does NOT read or write Rust.
//   - He talks in Bangla / Banglish: reply in Bangla when he writes Bangla/Banglish.
//   - ALWAYS give him the COMPLETE main.rs file (he replaces the whole file, he
//     does not hunt for lines to edit). Never ask him to paste code into cmd.
//   - Make ONLY the change he asked for. Do not touch the UI design or unrelated
//     features. Do not over-engineer. Keep answers short, step by step.
//   - Give him cmd commands step by step (see BUILD below).
//   - Only at the very END of a finished project remind him once to run
//     `cargo clean` (never after every build).
//   - The AI cannot compile for Windows in its sandbox, so code is not compiled
//     before delivery: be careful with crate APIs (see VERSIONS below).
//
// WHAT THE APP DOES
//   - Live camera preview (mirrored, like a mirror), 16:9 always.
//   - Photo -> %USERPROFILE%\Pictures\FastCam\photo_<unixtime>.jpg  (JPEG q92)
//   - Video -> %USERPROFILE%\Videos\FastCam\video_<unixtime>.mp4    (H.264 + AAC mic audio)
//   - Photo, video and preview all look the SAME (the frame is mirrored once in
//     camera_thread, the preview does not mirror again).
//   - Shortcuts (also shown inside the app with H):
//       SPACE photo | V start/stop video | M mic on/off while recording
//       O open Pictures folder | SHIFT+O open Videos folder
//       F11 fullscreen (ESC exits) | C next camera | Q best/light quality
//       B change filter (Normal, Bright, Gray, Warm) | H or F1 help panel
//   - Remembered between runs: filter, quality (best/light) and selected camera
//     (file %APPDATA%\FastCam\settings.txt). Press C once to choose a camera; it is remembered.
//   - Photos/videos never overwrite each other (unique file names).
//   - High-DPI screens: buttons and hint text scale with the Windows display scale.
//   - Best quality = the highest picture the camera really offers (we ask for up to 4K,
//     the camera answers with its own maximum). Q switches to a light 640x360 mode.
//   - If the camera is unplugged / taken by another app, the app keeps retrying and
//     reconnects by itself; a running video is saved and stopped.
//   - Window title is used as the status line (errors, "REC 00:12", etc.),
//     because the app is a windowed exe (no console).
//
// PROJECT FOLDER (E:\Rust Projects\fastcam) - give this whole folder as a zip
//   src\main.rs      this file (all the code)
//   Cargo.toml       dependencies (see VERSIONS)
//   Cargo.lock       exact dependency versions (keep it)
//   build.rs         uses `winresource` to embed icon.ico into the exe as
//                    Windows resource id 1 (Icon::from_resource(1, ..) uses it)
//   icon.ico         app icon (used by build.rs and the installer)
//   .cargo\config.toml  rustflags `+crt-static`: the exe has no VC++ runtime
//                    dependency, so it runs on other PCs without extra installs
//   FastCam.iss      Inno Setup script -> builds installer\FastCam_Setup.exe
//   installer\       output folder of the installer (final file for sharing)
//   target\          cargo build output (can be deleted with `cargo clean`)
//   .git, .gitignore git data
//
// BUILD (Windows cmd). Creates target\release\fastcam.exe and then the installer:
//   cargo build --release --manifest-path "E:\Rust Projects\fastcam\Cargo.toml" && for %I in ("C:\Program Files (x86)\Inno Setup 6\ISCC.exe" "%LOCALAPPDATA%\Programs\Inno Setup 6\ISCC.exe") do if exist %I %I "E:\Rust Projects\fastcam\FastCam.iss"
//   To replace the code from a downloaded file:
//   copy /Y "%USERPROFILE%\Downloads\main.rs" "E:\Rust Projects\fastcam\src\main.rs"
//
// VERSIONS (crate APIs differ a lot between versions - stay on these)
//   nokhwa 0.10 (feature input-native)  camera capture (Media Foundation)
//   winit 0.30                          window + events (ApplicationHandler API)
//   softbuffer 0.4                      draws pixels (u32 0x00RRGGBB) into the window
//   image 0.25 (only jpeg feature)      saves photos (JpegEncoder, ExtendedColorType)
//   windows 0.61                        Media Foundation (video/audio encoding)
//                                       and WASAPI (microphone)
//   Rust edition 2024.
//
// ARCHITECTURE (3 kinds of threads)
//   1) UI / main thread: struct App (winit). Draws the preview + buttons with
//      softbuffer, handles keyboard/mouse. No GPU, everything is CPU pixel code.
//   2) camera_thread: grabs frames from the webcam, crops to 16:9, mirrors,
//      applies the filter, hands the frame to the recording worker (if recording),
//      then publishes the newest frame in `Shared` and wakes the UI with
//      proxy.send_event(()). It also handles camera switch / quality switch.
//   3) Recording worker (start_recorder): its own thread. It creates the Recorder and
//      writes the frames it receives through a small queue (if the encoder is slow,
//      frames are dropped; the preview never waits). The microphone has one more thread (Job::run)
//      that converts mic audio to 48 kHz stereo 16-bit PCM and feeds the
//      MP4 writer.
//   The threads talk through: `Shared` (latest frame), `Ctl` (atomics + small
//   mutex-protected messages), and an mpsc channel (frames for the recording worker).
//
// FRAME FORMAT
//   Frames are RGB, 3 bytes per pixel, row after row (Shot.raw). The video
//   writer wants RGB32 bottom-up, which is why Recorder::write flips rows.
//
// KNOWN GOTCHAS
//   - Never `println!` for status: no console exists. Use post(&ctl, "text") to
//     show a message in the window title.
//   - Some webcams give black pictures or fail with MJPEG 1080p. open_cam opens the device ONCE
//     and picks the best entry from the camera's own format list (compatible_camera_formats);
//     the old 5-request ladder is only a fallback. Asking the driver for a format it cannot give
//     costs ~0.5 s per attempt: on a 720p webcam the old order lost ~1.6 s at every start.
//     Windows camera privacy must be ON for desktop apps.
//   - Video + audio timing uses one shared Instant (start) so they stay in sync;
//     audio is written ~150 ms behind the clock (LAG) and gaps are filled with silence.
//   - If the user switches camera/quality while recording it is refused on purpose.
//   - Settings file is plain text (filter=, light=, camera=); delete it to reset.
//   - panic = "abort" in release profile: any panic closes the app.
//
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use image::codecs::jpeg::JpegEncoder;
use image::{ExtendedColorType, ImageEncoder};
use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{ApiBackend, CameraFormat, CameraIndex, FrameFormat, RequestedFormat, RequestedFormatType};
use nokhwa::Camera;
use std::num::NonZeroU32;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{self, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::platform::windows::{IconExtWindows, WindowAttributesExtWindows, WindowExtWindows};
use winit::window::{CursorIcon, Fullscreen, Icon, Window, WindowId};

// Video + audio recording (Windows Media Foundation + WASAPI microphone).
// Public API: Recorder::new(path, w, h, hw, mute) -> write(frame) per video frame -> finish().
// Output is an .mp4 with H.264 video (30 fps) and AAC audio (if a microphone works).
mod rec {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};
    use windows::core::{Result, PCWSTR};
    use windows::Win32::Foundation::E_FAIL;
    use windows::Win32::Media::Audio::{
        eCapture, eConsole, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator,
        MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, WAVEFORMATEX,
    };
    use windows::Win32::Media::MediaFoundation::*;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED};

    // Audio constants. RATE = sample rate of the recorded audio (48 kHz).
    const RATE: u64 = 48_000;
    const LAG: u64 = 7_200; // 150 ms: audio is written this far behind the clock
    const GAP: u64 = 2_880; // 60 ms: bigger hole than this is filled with silence

    // Wrapper so the Media Foundation sink writer can be shared between threads (video + audio).
    struct W(IMFSinkWriter);
    unsafe impl Send for W {}
    unsafe impl Sync for W {}

    // Audio converter: takes whatever the microphone gives (any rate, 1-2+ channels,
    // 16/24/32-bit int or float) and converts it to 48 kHz stereo i16 with linear resampling.
    struct Conv {
        rate: f64,
        ch: usize,
        bytes: usize,
        float: bool,
        block: usize,
        pos: f64,
        prev: [f32; 2],
        have: bool,
    }

    impl Conv {
        fn sample(&self, b: &[u8], o: usize) -> f32 {
            match (self.float, self.bytes) {
                (true, 4) => f32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]),
                (false, 2) => i16::from_le_bytes([b[o], b[o + 1]]) as f32 / 32768.0,
                (false, 3) => {
                    let v = ((b[o + 2] as i8 as i32) << 16) | ((b[o + 1] as i32) << 8) | b[o] as i32;
                    v as f32 / 8388608.0
                }
                (false, 4) => {
                    i32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]) as f32 / 2147483648.0
                }
                _ => 0.0,
            }
        }

        // device bytes -> 48 kHz stereo i16 (interleaved)
        fn push(&mut self, bytes: &[u8], out: &mut Vec<i16>) {
            let frames = bytes.len() / self.block;
            let mut inp: Vec<[f32; 2]> = Vec::with_capacity(frames + 1);
            if self.have {
                inp.push(self.prev);
            }
            for f in 0..frames {
                let o = f * self.block;
                let l = self.sample(bytes, o);
                let r = if self.ch > 1 { self.sample(bytes, o + self.bytes) } else { l };
                inp.push([l, r]);
            }
            let n = inp.len();
            let step = self.rate / RATE as f64;
            while n > 0 && (self.pos as usize) + 1 < n {
                let i = self.pos as usize;
                let fr = (self.pos - i as f64) as f32;
                for c in 0..2 {
                    let v = inp[i][c] * (1.0 - fr) + inp[i + 1][c] * fr;
                    out.push((v.clamp(-1.0, 1.0) * 32767.0) as i16);
                }
                self.pos += step;
            }
            if n > 0 {
                self.pos -= (n - 1) as f64;
                self.prev = inp[n - 1];
                self.have = true;
            }
        }
    }

    // One open microphone: WASAPI client + capture client + its converter.
    struct Src {
        client: IAudioClient,
        cap: IAudioCaptureClient,
        conv: Conv,
    }

    // Everything the audio thread needs. Job::run is the audio thread's main loop.
    struct Job {
        mic: Src,
        mute: Arc<AtomicBool>,
        stop: Arc<AtomicBool>,
        writer: Arc<Mutex<W>>,
        stream: u32,
        start: Instant,
    }
    unsafe impl Send for Job {}

    // Opens the default microphone (shared mode). Called only when recording starts,
    // so the mic is not in use while just previewing. Returns Err if blocked/missing.
    fn open_mic() -> Result<Src> {
        unsafe {
            let en: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            let dev = en.GetDefaultAudioEndpoint(eCapture, eConsole)?;
            let client: IAudioClient = dev.Activate(CLSCTX_ALL, None)?;
            let fmt: *mut WAVEFORMATEX = client.GetMixFormat()?;
            let f = std::ptr::read_unaligned(fmt);
            let tag = f.wFormatTag;
            let ch = f.nChannels as usize;
            let rate = f.nSamplesPerSec;
            let bits = f.wBitsPerSample as usize;
            let block = f.nBlockAlign as usize;
            let mut float = tag == 3;
            if tag == 0xFFFE {
                let d1 = std::ptr::read_unaligned((fmt as *const u8).add(24) as *const u32);
                float = d1 == 3;
            }
            let bytes = bits / 8;
            if ch == 0 || rate == 0 || bytes == 0 || block == 0 || ch.min(2) * bytes > block {
                return Err(E_FAIL.into());
            }
            client.Initialize(AUDCLNT_SHAREMODE_SHARED, 0, 10_000_000, 0, fmt, None)?;
            let cap: IAudioCaptureClient = client.GetService()?;
            Ok(Src {
                client,
                cap,
                conv: Conv {
                    rate: rate as f64,
                    ch,
                    bytes,
                    float,
                    block,
                    pos: 0.0,
                    prev: [0.0; 2],
                    have: false,
                },
            })
        }
    }

    // Reads all waiting microphone packets, converts them, and appends to buf.
    // If the mic delivered less than the clock expects (a hole bigger than GAP), the hole is filled with silence.
    fn drain(s: &mut Src, buf: &mut Vec<i16>, written: u64, now: u64) {
        let mut new: Vec<i16> = Vec::new();
        unsafe {
            loop {
                let n = match s.cap.GetNextPacketSize() {
                    Ok(n) => n,
                    Err(_) => break,
                };
                if n == 0 {
                    break;
                }
                let mut data: *mut u8 = std::ptr::null_mut();
                let mut frames = 0u32;
                let mut flags = 0u32;
                if s.cap.GetBuffer(&mut data, &mut frames, &mut flags, None, None).is_err() {
                    break;
                }
                let len = frames as usize * s.conv.block;
                if flags & 2 != 0 || data.is_null() {
                    let z = vec![0u8; len];
                    s.conv.push(&z, &mut new);
                } else {
                    let bytes = std::slice::from_raw_parts(data, len);
                    s.conv.push(bytes, &mut new);
                }
                let _ = s.cap.ReleaseBuffer(frames);
            }
        }
        if new.is_empty() {
            return;
        }
        let nf = (new.len() / 2) as u64;
        let end = written + (buf.len() / 2) as u64;
        if now > nf + end + GAP {
            let z = (now - nf - end) as usize;
            buf.resize(buf.len() + z * 2, 0);
        }
        buf.extend_from_slice(&new);
        if buf.len() / 2 > 24_000 {
            let drop = buf.len() / 2 - 14_400;
            buf.drain(..drop * 2);
        }
    }

    // Wraps a block of PCM samples in a Media Foundation sample and writes it to the audio stream.
    fn write_pcm(writer: &Mutex<W>, stream: u32, pcm: &[i16], t: i64, d: i64) -> Result<()> {
        unsafe {
            let len = (pcm.len() * 2) as u32;
            let buf = MFCreateMemoryBuffer(len)?;
            let mut ptr: *mut u8 = std::ptr::null_mut();
            buf.Lock(&mut ptr, None, None)?;
            std::ptr::copy_nonoverlapping(pcm.as_ptr() as *const u8, ptr, len as usize);
            buf.Unlock()?;
            buf.SetCurrentLength(len)?;
            let sample = MFCreateSample()?;
            sample.AddBuffer(&buf)?;
            sample.SetSampleTime(t)?;
            sample.SetSampleDuration(d)?;
            if let Ok(g) = writer.lock() {
                g.0.WriteSample(stream, &sample)?;
            }
        }
        Ok(())
    }

    // Audio thread loop: every ~15 ms read the mic, then write audio up to (now - LAG).
    // When stop is set it flushes everything up to now and exits. Mute = write silence.
    impl Job {
        fn run(mut self) {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            }
            let mut written: u64 = 0;
            let mut mbuf: Vec<i16> = Vec::new();
            loop {
                std::thread::sleep(Duration::from_millis(15));
                let stop = self.stop.load(Ordering::Relaxed);
                let now = (self.start.elapsed().as_nanos() * RATE as u128 / 1_000_000_000) as u64;
                drain(&mut self.mic, &mut mbuf, written, now);
                if self.mute.load(Ordering::Relaxed) {
                    mbuf.clear();
                }
                let target = if stop { now } else { now.saturating_sub(LAG) };
                if target > written {
                    let n = (target - written) as usize;
                    let ma = (mbuf.len() / 2).min(n);
                    let mut out: Vec<i16> = vec![0; n * 2];
                    out[..ma * 2].copy_from_slice(&mbuf[..ma * 2]);
                    mbuf.drain(..ma * 2);
                    let t = (written as u128 * 10_000_000 / RATE as u128) as i64;
                    let t2 = (target as u128 * 10_000_000 / RATE as u128) as i64;
                    let _ = write_pcm(&self.writer, self.stream, &out, t, t2 - t);
                    written = target;
                }
                if stop {
                    break;
                }
            }
            unsafe {
                let _ = self.mic.client.Stop();
            }
        }
    }

    // Handle for one recording. w/h are even numbers (H.264 needs even sizes).
    // note = optional message for the UI (e.g. mic blocked, video only).
    pub struct Recorder {
        writer: Arc<Mutex<W>>,
        stream: u32,
        w: usize,
        h: usize,
        start: Instant,
        stop: Arc<AtomicBool>,
        audio: Option<JoinHandle<()>>,
        pub note: Option<String>,
    }

    // Creates the MP4 sink writer: H.264 video stream (bitrate by resolution: 2.5/5/8 Mbit/s),
    // RGB32 input, and optionally an AAC audio stream with PCM input. hw = allow hardware encoder.
    fn build(path: &str, w: usize, h: usize, hw: bool, audio: bool) -> Result<(IMFSinkWriter, u32, Option<u32>)> {
        unsafe {
            let mut attrs: Option<IMFAttributes> = None;
            MFCreateAttributes(&mut attrs, 1)?;
            let attrs = attrs.unwrap();
            attrs.SetUINT32(&MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS, hw as u32)?;
            let wide: Vec<u16> = path.encode_utf16().chain(std::iter::once(0)).collect();
            let writer = MFCreateSinkWriterFromURL(PCWSTR(wide.as_ptr()), None, Some(&attrs))?;

            let px = w * h;
            let br: u32 = if px > 3_000_000 {
                20_000_000
            } else if px > 1_200_000 {
                8_000_000
            } else if px > 400_000 {
                5_000_000
            } else {
                2_500_000
            };
            let out = MFCreateMediaType()?;
            out.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            out.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_H264)?;
            out.SetUINT32(&MF_MT_AVG_BITRATE, br)?;
            out.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            out.SetUINT64(&MF_MT_FRAME_SIZE, ((w as u64) << 32) | h as u64)?;
            out.SetUINT64(&MF_MT_FRAME_RATE, (30u64 << 32) | 1)?;
            out.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
            let stream = writer.AddStream(&out)?;

            let inp = MFCreateMediaType()?;
            inp.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Video)?;
            inp.SetGUID(&MF_MT_SUBTYPE, &MFVideoFormat_RGB32)?;
            inp.SetUINT32(&MF_MT_INTERLACE_MODE, MFVideoInterlace_Progressive.0 as u32)?;
            inp.SetUINT64(&MF_MT_FRAME_SIZE, ((w as u64) << 32) | h as u64)?;
            inp.SetUINT64(&MF_MT_FRAME_RATE, (30u64 << 32) | 1)?;
            inp.SetUINT64(&MF_MT_PIXEL_ASPECT_RATIO, (1u64 << 32) | 1)?;
            writer.SetInputMediaType(stream, &inp, None)?;

            let mut astream = None;
            if audio {
                let ao = MFCreateMediaType()?;
                ao.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
                ao.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_AAC)?;
                ao.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
                ao.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000)?;
                ao.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
                ao.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 16_000)?;
                ao.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 1)?;
                ao.SetUINT32(&MF_MT_AAC_AUDIO_PROFILE_LEVEL_INDICATION, 0x29)?;
                let a = writer.AddStream(&ao)?;

                let ai = MFCreateMediaType()?;
                ai.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
                ai.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_PCM)?;
                ai.SetUINT32(&MF_MT_AUDIO_BITS_PER_SAMPLE, 16)?;
                ai.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, 48_000)?;
                ai.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, 2)?;
                ai.SetUINT32(&MF_MT_AUDIO_BLOCK_ALIGNMENT, 4)?;
                ai.SetUINT32(&MF_MT_AUDIO_AVG_BYTES_PER_SECOND, 192_000)?;
                writer.SetInputMediaType(a, &ai, None)?;
                astream = Some(a);
            }
            writer.BeginWriting()?;
            Ok((writer, stream, astream))
        }
    }

    // Recorder::new tries the mic first, then builds the writer (falls back to video-only if audio fails).
    // Called from a helper thread (see camera_thread) with hardware encoding first, then software.
    impl Recorder {
        pub fn new(path: &str, fw: u32, fh: u32, hw: bool, mute: Arc<AtomicBool>) -> Result<Recorder> {
            let w = (fw & !1) as usize;
            let h = (fh & !1) as usize;
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                MFStartup(0x0002_0070, 0)?;
            }

            // open the microphone only now, at recording start
            let mut note: Option<String> = None;
            let mut mic = match open_mic() {
                Ok(s) => Some(s),
                Err(e) => {
                    note = Some(if e.code().0 as u32 == 0x8007_0005 {
                        "mic blocked: Settings > Privacy > Microphone".to_string()
                    } else {
                        "no microphone, video only".to_string()
                    });
                    None
                }
            };

            let (writer, stream, astream) = match build(path, w, h, hw, mic.is_some()) {
                Ok(x) => x,
                Err(e) => {
                    if mic.is_none() {
                        return Err(e);
                    }
                    mic = None;
                    note = Some("audio encoder failed, video only".to_string());
                    build(path, w, h, hw, false)?
                }
            };

            let writer = Arc::new(Mutex::new(W(writer)));
            let stop = Arc::new(AtomicBool::new(false));

            unsafe {
                if let Some(m) = mic.as_ref() {
                    let _ = m.client.Start();
                }
            }
            let start = Instant::now();

            let mut audio = None;
            if let (Some(m), Some(a)) = (mic, astream) {
                let job = Job { mic: m, mute, stop: stop.clone(), writer: writer.clone(), stream: a, start };
                audio = Some(std::thread::spawn(move || job.run()));
            }

            Ok(Recorder { writer, stream, w, h, start, stop, audio, note })
        }

        // Writes one video frame. Converts RGB -> RGB32 (0x00RRGGBB) and flips rows (MF wants bottom-up).
        // Timestamp = time since recording start.
        // raw = RGB bytes (3 per pixel), fw = frame width in pixels
        pub fn write(&mut self, raw: &[u8], fw: usize, t: Duration) -> Result<()> {
            let (w, h) = (self.w, self.h);
            if raw.len() < h * fw * 3 {
                return Ok(());
            }
            let len = (w * h * 4) as u32;
            unsafe {
                let buf = MFCreateMemoryBuffer(len)?;
                let mut ptr: *mut u8 = std::ptr::null_mut();
                buf.Lock(&mut ptr, None, None)?;
                let dst = std::slice::from_raw_parts_mut(ptr as *mut u32, w * h);
                for y in 0..h {
                    let sy = h - 1 - y;
                    let src = &raw[sy * fw * 3..sy * fw * 3 + w * 3];
                    let drow = &mut dst[y * w..y * w + w];
                    for (d, p) in drow.iter_mut().zip(src.chunks_exact(3)) {
                        *d = ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32;
                    }
                }
                buf.Unlock()?;
                buf.SetCurrentLength(len)?;
                let sample = MFCreateSample()?;
                sample.AddBuffer(&buf)?;
                sample.SetSampleTime((t.as_nanos() / 100) as i64)?;
                sample.SetSampleDuration(333_333)?;
                if let Ok(g) = self.writer.lock() {
                    g.0.WriteSample(self.stream, &sample)?;
                }
            }
            Ok(())
        }

        // moment the recording clock started (frames are timed relative to it)
        pub fn start(&self) -> Instant {
            self.start
        }

        pub fn finish(mut self) {
            self.stop.store(true, Ordering::Relaxed);
            if let Some(h) = self.audio.take() {
                let _ = h.join();
            }
            unsafe {
                if let Ok(g) = self.writer.lock() {
                    let _ = g.0.Finalize();
                }
            }
        }
    }
}

// One camera frame: width, height and RGB bytes (w*h*3). Already cropped to 16:9, mirrored and filtered.
struct Shot {
    w: u32,
    h: u32,
    raw: Vec<u8>,
}

// Arc inside, so the UI and photo code can grab the latest frame without holding the lock
// Latest frame shared between camera thread (writer) and UI (reader).
type Shared = Arc<Mutex<Option<Arc<Shot>>>>;

// Control block shared by UI and camera thread (all cheap atomics / small mutexes).
// recording = user wants recording on; busy = recorder still starting/finishing (window waits for it on close);
// mute = mic muted; req = request flags for camera thread; filter = current filter;
// note = message about recording (shown in title); msg = one-shot status text for the title.
struct Ctl {
    recording: AtomicBool,
    busy: AtomicBool,
    mute: Arc<AtomicBool>,
    req: AtomicU8,    // bit 1 = next camera, bit 2 = toggle best/light quality
    filter: AtomicU8, // 0 normal, 1 bright, 2 gray, 3 warm
    light: AtomicBool, // true = light quality (640x360)
    cam: AtomicU8,    // selected camera index
    note: Mutex<Option<String>>,
    msg: Mutex<Option<String>>,
}

// UI constants: R = button radius in pixels; PRESS_MS / FLASH_MS = press animation / white flash
// time; FILTERS = filter names; HINT = text of the top-left hint.
const R: i32 = 28;
const PRESS_MS: f32 = 220.0;
const FLASH_MS: f32 = 160.0;
const FILTERS: [&str; 4] = ["Normal", "Bright", "Gray", "Warm"];
const HINT: &str = "H = SHORTCUTS";

// Returns %USERPROFILE%\<sub>\FastCam (sub = "Pictures" or "Videos").
fn user_dir(sub: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("USERPROFILE").unwrap_or(".".into()))
        .join(sub)
        .join("FastCam")
}

// Current unix time in seconds (used in file names).
fn stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// Windows display scale (100% = 1.0, 150% = 1.5 ...) used to size buttons and hint text.
fn ui_scale(win: &Window) -> f32 {
    (win.scale_factor() as f32).clamp(1.0, 4.0)
}

// Creates a NEW empty file Pictures\FastCam\photo_<time>.jpg (or Videos\...\video_<time>.mp4).
// If the name exists already (two photos in the same second) it adds _1, _2 ... so nothing is overwritten.
fn unique_path(dir: &std::path::Path, prefix: &str, ext: &str) -> Option<std::path::PathBuf> {
    let _ = std::fs::create_dir_all(dir);
    let s = stamp();
    for n in 0..1000u32 {
        let name = if n == 0 {
            format!("{prefix}_{s}.{ext}")
        } else {
            format!("{prefix}_{s}_{n}.{ext}")
        };
        let p = dir.join(name);
        if std::fs::OpenOptions::new().write(true).create_new(true).open(&p).is_ok() {
            return Some(p);
        }
    }
    None
}

// Settings file: %APPDATA%\FastCam\settings.txt  (filter, quality and camera are remembered).
fn settings_path() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("APPDATA").unwrap_or(".".into()))
        .join("FastCam")
        .join("settings.txt")
}

// returns (filter, light quality, camera index)
fn load_settings() -> (u8, bool, u8) {
    let (mut filter, mut light, mut cam) = (0u8, false, 0u8);
    if let Ok(text) = std::fs::read_to_string(settings_path()) {
        for line in text.lines() {
            if let Some((k, v)) = line.split_once('=') {
                let v = v.trim();
                match k.trim() {
                    "filter" => {
                        if let Ok(f) = v.parse::<u8>() {
                            if (f as usize) < FILTERS.len() {
                                filter = f;
                            }
                        }
                    }
                    "light" => light = v == "1",
                    "camera" => {
                        if let Ok(c) = v.parse::<u8>() {
                            cam = c;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
    (filter, light, cam)
}

fn save_settings(ctl: &Ctl) {
    let p = settings_path();
    if let Some(d) = p.parent() {
        let _ = std::fs::create_dir_all(d);
    }
    let text = format!(
        "filter={}\nlight={}\ncamera={}\n",
        ctl.filter.load(Ordering::Relaxed),
        ctl.light.load(Ordering::Relaxed) as u8,
        ctl.cam.load(Ordering::Relaxed)
    );
    let _ = std::fs::write(p, text);
}

// Blends colour s over d (0x00RRGGBB) with alpha a (0..1).
fn mix(d: u32, s: u32, a: f32) -> u32 {
    let a = a.clamp(0.0, 1.0);
    let ch = |sh: u32| {
        let dv = ((d >> sh) & 255) as f32;
        let sv = ((s >> sh) & 255) as f32;
        ((dv + (sv - dv) * a) as u32) & 255
    };
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

// Anti-aliased filled circle drawn straight into the pixel buffer.
fn aa_circle(b: &mut [u32], ww: i32, wh: i32, cx: f32, cy: f32, r: f32, c: u32, alpha: f32) {
    let x0 = ((cx - r - 1.0).floor() as i32).max(0);
    let x1 = ((cx + r + 1.0).ceil() as i32).min(ww - 1);
    let y0 = ((cy - r - 1.0).floor() as i32).max(0);
    let y1 = ((cy + r + 1.0).ceil() as i32).min(wh - 1);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let cov = (r + 0.5 - (dx * dx + dy * dy).sqrt()).clamp(0.0, 1.0) * alpha;
            if cov > 0.0 {
                let i = (y * ww + x) as usize;
                b[i] = mix(b[i], c, cov);
            }
        }
    }
}

// Anti-aliased rounded rectangle drawn into the pixel buffer.
fn aa_rrect(b: &mut [u32], ww: i32, wh: i32, cx: f32, cy: f32, hw: f32, hh: f32, rad: f32, c: u32, alpha: f32) {
    let x0 = ((cx - hw - 1.0).floor() as i32).max(0);
    let x1 = ((cx + hw + 1.0).ceil() as i32).min(ww - 1);
    let y0 = ((cy - hh - 1.0).floor() as i32).max(0);
    let y1 = ((cy + hh + 1.0).ceil() as i32).min(wh - 1);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let dx = x as f32 + 0.5 - cx;
            let dy = y as f32 + 0.5 - cy;
            let qx = dx.abs() - (hw - rad);
            let qy = dy.abs() - (hh - rad);
            let d = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - rad;
            let cov = (0.5 - d).clamp(0.0, 1.0) * alpha;
            if cov > 0.0 {
                let i = (y * ww + x) as usize;
                b[i] = mix(b[i], c, cov);
            }
        }
    }
}

// Positions of the two round buttons (photo, video), bottom centre. u = UI scale (high-DPI screens).
// Used for drawing AND mouse hit-testing.
fn buttons(ww: i32, wh: i32, u: f32) -> ((i32, i32), (i32, i32)) {
    let cy = wh - (46.0 * u) as i32;
    let dx = (44.0 * u) as i32;
    ((ww / 2 - dx, cy), (ww / 2 + dx, cy))
}

// Draws the two buttons (photo = camera icon, video = red record / stop icon) with press animation.
// u = UI scale: every size below is multiplied by it.
fn draw_ui(b: &mut [u32], ww: i32, wh: i32, rec: bool, press: Option<(u8, f32)>, u: f32) {
    let (p, v) = buttons(ww, wh, u);
    let pi = std::f32::consts::PI;
    let k = |id: u8| -> f32 {
        match press {
            Some((i, t)) if i == id => (pi * t).sin(),
            _ => 0.0,
        }
    };
    let rr = R as f32;
    let white = 0x00FFFFFF;
    let dark = 0x00333333;

    for (id, c) in [(0u8, p), (1u8, v)] {
        if let Some((i, t)) = press {
            if i == id {
                aa_circle(b, ww, wh, c.0 as f32, c.1 as f32, rr * u * (1.0 + 0.8 * t), white, 0.35 * (1.0 - t));
            }
        }
    }

    {
        let s = (1.0 - 0.16 * k(0)) * u;
        let (cx, cy) = (p.0 as f32, p.1 as f32);
        let fill = mix(0x00F2F2F2, 0x00B0B0B0, k(0));
        aa_circle(b, ww, wh, cx, cy, (rr + 2.0) * s, 0x00101010, 0.55);
        aa_circle(b, ww, wh, cx, cy, rr * s, fill, 1.0);
        aa_rrect(b, ww, wh, cx, cy + 1.0 * s, 14.0 * s, 9.0 * s, 3.0 * s, dark, 1.0);
        aa_rrect(b, ww, wh, cx, cy - 8.0 * s, 6.0 * s, 3.0 * s, 1.5 * s, dark, 1.0);
        aa_circle(b, ww, wh, cx, cy + 1.0 * s, 6.5 * s, fill, 1.0);
        aa_circle(b, ww, wh, cx, cy + 1.0 * s, 3.5 * s, dark, 1.0);
    }

    {
        let s = (1.0 - 0.16 * k(1)) * u;
        let (cx, cy) = (v.0 as f32, v.1 as f32);
        let fill = mix(0x00E03030, 0x00A02020, k(1));
        aa_circle(b, ww, wh, cx, cy, (rr + 2.0) * s, 0x00101010, 0.55);
        aa_circle(b, ww, wh, cx, cy, rr * s, fill, 1.0);
        if rec {
            aa_rrect(b, ww, wh, cx, cy, 9.0 * s, 9.0 * s, 3.0 * s, white, 1.0);
        } else {
            aa_rrect(b, ww, wh, cx - 5.0 * s, cy, 10.0 * s, 8.0 * s, 3.0 * s, white, 1.0);
            aa_rrect(b, ww, wh, cx + 11.5 * s, cy, 4.5 * s, 5.0 * s, 1.5 * s, white, 1.0);
        }
    }
}

// Tiny 5x7 pixel font (A-Z, 0-9 and + - / : = ?). Only for the hint and help panel.
// To show a new character, add it here.
fn glyph(c: char) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        'A' => [14, 17, 17, 31, 17, 17, 17],
        'B' => [30, 17, 17, 30, 17, 17, 30],
        'C' => [14, 17, 16, 16, 16, 17, 14],
        'D' => [30, 17, 17, 17, 17, 17, 30],
        'E' => [31, 16, 16, 30, 16, 16, 31],
        'F' => [31, 16, 16, 30, 16, 16, 16],
        'G' => [14, 17, 16, 23, 17, 17, 15],
        'H' => [17, 17, 17, 31, 17, 17, 17],
        'I' => [14, 4, 4, 4, 4, 4, 14],
        'J' => [7, 2, 2, 2, 2, 18, 12],
        'K' => [17, 18, 20, 24, 20, 18, 17],
        'L' => [16, 16, 16, 16, 16, 16, 31],
        'M' => [17, 27, 21, 21, 17, 17, 17],
        'N' => [17, 25, 21, 19, 17, 17, 17],
        'O' => [14, 17, 17, 17, 17, 17, 14],
        'P' => [30, 17, 17, 30, 16, 16, 16],
        'Q' => [14, 17, 17, 17, 21, 18, 13],
        'R' => [30, 17, 17, 30, 20, 18, 17],
        'S' => [15, 16, 16, 14, 1, 1, 30],
        'T' => [31, 4, 4, 4, 4, 4, 4],
        'U' => [17, 17, 17, 17, 17, 17, 14],
        'V' => [17, 17, 17, 17, 17, 10, 4],
        'W' => [17, 17, 17, 21, 21, 27, 17],
        'X' => [17, 17, 10, 4, 10, 17, 17],
        'Y' => [17, 17, 10, 4, 4, 4, 4],
        'Z' => [31, 1, 2, 4, 8, 16, 31],
        '0' => [14, 17, 19, 21, 25, 17, 14],
        '1' => [4, 12, 4, 4, 4, 4, 14],
        '2' => [14, 17, 1, 2, 4, 8, 31],
        '3' => [30, 1, 1, 14, 1, 1, 30],
        '4' => [2, 6, 10, 18, 31, 2, 2],
        '5' => [31, 16, 30, 1, 1, 17, 14],
        '6' => [6, 8, 16, 30, 17, 17, 14],
        '7' => [31, 1, 2, 4, 8, 8, 8],
        '8' => [14, 17, 17, 14, 17, 17, 14],
        '9' => [14, 17, 17, 15, 1, 2, 12],
        '+' => [0, 4, 4, 31, 4, 4, 0],
        '-' => [0, 0, 0, 31, 0, 0, 0],
        '/' => [1, 2, 2, 4, 8, 8, 16],
        ':' => [0, 4, 0, 0, 0, 4, 0],
        '=' => [0, 0, 31, 0, 31, 0, 0],
        '?' => [14, 17, 1, 2, 4, 0, 4],
        _ => [0; 7],
    }
}

// Draws text with the pixel font at integer scale sc.
fn draw_text(b: &mut [u32], ww: usize, wh: usize, x: usize, y: usize, s: &str, sc: usize, col: u32) {
    let mut cx = x;
    for ch in s.chars() {
        let g = glyph(ch);
        for (r, row) in g.iter().enumerate() {
            for c in 0..5usize {
                if *row & (16u8 >> c) != 0 {
                    let x0 = cx + c * sc;
                    let y0 = y + r * sc;
                    for yy in y0..(y0 + sc).min(wh) {
                        for xx in x0..(x0 + sc).min(ww) {
                            b[yy * ww + xx] = col;
                        }
                    }
                }
            }
        }
        cx += 6 * sc;
    }
}

// Text scale of the hint (bigger on high-DPI screens).
fn hint_scale(u: f32) -> usize {
    ((2.0 * u).round() as usize).max(2)
}

// Bottom-right corner (x, y) of the clickable hint area, top-left corner of the window.
fn hint_end(u: f32) -> (i32, i32) {
    let sc = hint_scale(u);
    (
        (10.0 * u) as i32 + (HINT.len() * 6 * sc) as i32 + 6,
        (8.0 * u) as i32 + (7 * sc) as i32 + 6,
    )
}

// Draws the small "H = SHORTCUTS" text (with a black shadow) in the top-left corner.
fn draw_hint(b: &mut [u32], ww: usize, wh: usize, u: f32) {
    let sc = hint_scale(u);
    let (x, y) = ((10.0 * u) as usize, (8.0 * u) as usize);
    let sh = (sc / 2).max(1);
    draw_text(b, ww, wh, x + sh, y + sh, HINT, sc, 0x00000000);
    draw_text(b, ww, wh, x, y, HINT, sc, 0x00FFFFFF);
}

// Draws the shortcut help panel. Keep this list in sync with the key handling in window_event.
fn draw_help(b: &mut [u32], ww: usize, wh: usize) {
    const L: [&str; 12] = [
        "SHORTCUTS",
        "",
        "SPACE  TAKE PHOTO",
        "V  START / STOP VIDEO",
        "M  MIC ON / OFF WHILE REC",
        "O  OPEN PHOTOS FOLDER",
        "SHIFT+O  OPEN VIDEOS FOLDER",
        "F11  FULL SCREEN / ESC EXIT",
        "C  NEXT CAMERA",
        "Q  BEST / LIGHT QUALITY",
        "B  CHANGE FILTER",
        "H  SHOW / HIDE THIS HELP",
    ];
    let sc = (ww.saturating_sub(24) / (27 * 6))
        .min(wh.saturating_sub(44) / (L.len() * 8))
        .clamp(1, 4);
    let lh = 8 * sc;
    let pw = 27 * 6 * sc + 24;
    let ph = L.len() * lh + 24;
    let x0 = ww.saturating_sub(pw) / 2;
    let y0 = 10usize;
    for y in y0..(y0 + ph).min(wh) {
        for x in x0..(x0 + pw).min(ww) {
            let i = y * ww + x;
            b[i] = mix(b[i], 0x00000000, 0.8);
        }
    }
    for (i, line) in L.iter().enumerate() {
        let col = if i == 0 { 0x00FFD060 } else { 0x00FFFFFF };
        draw_text(b, ww, wh, x0 + 12, y0 + 12 + i * lh, line, sc, col);
    }
}

// Colour filters on RGB bytes: 1 bright, 2 gray, 3 warm (0 = none).
fn apply_filter(raw: &mut [u8], f: u8) {
    match f {
        1 => {
            for v in raw.iter_mut() {
                *v = ((*v as u16 * 5 / 4) + 12).min(255) as u8;
            }
        }
        2 => {
            for p in raw.chunks_exact_mut(3) {
                let y = ((p[0] as u32 * 77 + p[1] as u32 * 151 + p[2] as u32 * 28) >> 8) as u8;
                p[0] = y;
                p[1] = y;
                p[2] = y;
            }
        }
        3 => {
            for p in raw.chunks_exact_mut(3) {
                p[0] = (p[0] as u16 * 9 / 8).min(255) as u8;
                p[2] = (p[2] as u16 * 7 / 8) as u8;
            }
        }
        _ => {}
    }
}

// Picks the best entry out of the camera's own format list.
// best quality = biggest picture, MJPEG first, and at least 24 fps when the camera offers it;
// light quality = the size nearest to 640x360.
fn pick_format(all: &[CameraFormat], light: bool) -> Option<CameraFormat> {
    let smooth: Vec<CameraFormat> = all.iter().copied().filter(|f| f.frame_rate() >= 24).collect();
    let base = if smooth.is_empty() { all.to_vec() } else { smooth };
    let mjpeg: Vec<CameraFormat> = base
        .iter()
        .copied()
        .filter(|f| f.format() == FrameFormat::MJPEG)
        .collect();
    let pool = if mjpeg.is_empty() { base } else { mjpeg };
    if light {
        pool.into_iter()
            .min_by_key(|f| (f.width() as u64 * f.height() as u64).abs_diff(640 * 360))
    } else {
        pool.into_iter()
            .max_by_key(|f| (f.width() as u64 * f.height() as u64, f.frame_rate()))
    }
}

// set_camera_format is deprecated in nokhwa 0.10, but it is the only direct "use exactly this format"
// call (set_camera_requset would negotiate again and could reject a format from the list).
#[allow(deprecated)]
fn set_format(cam: &mut Camera, f: CameraFormat) -> bool {
    cam.set_camera_format(f).is_ok()
}

// Opens the camera. FAST PATH: one device open, then read the camera's own format list (costs ~0 ms)
// and set the best entry from it. The old way asked the driver for a format it cannot give, which
// costs ~0.5 s per failed attempt — on a 720p webcam that happened three times before one worked.
// If a camera rejects the fast path, the old request ladder below still works, just slower.
fn open_cam(idx: usize, light: bool) -> Option<Camera> {
    let index = CameraIndex::Index(idx as u32);
    if let Ok(mut cam) = Camera::new(
        index.clone(),
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::None),
    ) {
        let picked = cam
            .compatible_camera_formats()
            .ok()
            .and_then(|list| pick_format(&list, light));
        let set = match picked {
            Some(f) => set_format(&mut cam, f),
            None => true, // no format list from this backend: keep what the camera opened with
        };
        if set && cam.open_stream().is_ok() {
            return Some(cam);
        }
        let _ = cam.stop_stream();
    }

    // Slow fallback for cameras that fail the fast path (same requests as before).
    let (w, h) = if light { (640, 360) } else { (3840, 2160) };
    let tries = [
        RequestedFormatType::Closest(CameraFormat::new_from(w, h, FrameFormat::MJPEG, 30)),
        RequestedFormatType::Closest(CameraFormat::new_from(w, h, FrameFormat::YUYV, 30)),
        RequestedFormatType::Closest(CameraFormat::new_from(w, h, FrameFormat::NV12, 30)),
        RequestedFormatType::AbsoluteHighestFrameRate,
        RequestedFormatType::None,
    ];
    for t in tries {
        let fmt = RequestedFormat::new::<RgbFormat>(t);
        if let Ok(mut cam) = Camera::new(CameraIndex::Index(idx as u32), fmt) {
            if cam.open_stream().is_ok() {
                return Some(cam);
            }
        }
    }
    None
}

// Shows a short status message in the window title (UI picks it up on the next frame event).
fn post(ctl: &Ctl, s: &str) {
    *ctl.msg.lock().unwrap() = Some(s.to_string());
}

// Recording worker thread. Created when recording starts. It makes the video file (Videos\FastCam),
// builds the Recorder (hardware encoder first, then software) and writes every frame it receives.
// The frame queue is small: if encoding is slow, frames are dropped instead of freezing the preview.
// When the sender is dropped (recording stopped) it finishes the mp4 and clears `busy`.
fn start_recorder(ctl: Arc<Ctl>, w: u32, h: u32) -> mpsc::SyncSender<(Instant, Arc<Shot>)> {
    let (tx, rx) = mpsc::sync_channel::<(Instant, Arc<Shot>)>(6);
    std::thread::spawn(move || {
        let path = match unique_path(&user_dir("Videos"), "video", "mp4") {
            Some(p) => p.to_string_lossy().to_string(),
            None => {
                post(&ctl, "video failed to start");
                ctl.recording.store(false, Ordering::Relaxed);
                ctl.busy.store(false, Ordering::Relaxed);
                return;
            }
        };
        let mute = ctl.mute.clone();
        let made = rec::Recorder::new(&path, w, h, true, mute.clone())
            .or_else(|_| rec::Recorder::new(&path, w, h, false, mute))
            .ok();
        let mut r = match made {
            Some(r) => r,
            None => {
                post(&ctl, "video failed to start");
                ctl.recording.store(false, Ordering::Relaxed);
                ctl.busy.store(false, Ordering::Relaxed);
                return;
            }
        };
        if let Some(n) = r.note.take() {
            *ctl.note.lock().unwrap() = Some(n);
        }
        let start = r.start();
        for (t, shot) in rx.iter() {
            // skip frames captured before the recorder was ready, or with a different size
            if t < start || shot.w != w || shot.h != h {
                continue;
            }
            let _ = r.write(&shot.raw, shot.w as usize, t.duration_since(start));
        }
        r.finish();
        ctl.busy.store(false, Ordering::Relaxed);
    });
    tx
}

// Camera thread main loop. Each round:
//  (1) handle requests from the UI (next camera / quality switch) and save the new setting
//  (2) if there is no camera (not found, unplugged, used by another app) retry every 0.7 s
//  (3) read a frame and decode it to RGB (many read errors in a row = camera lost -> reconnect)
//  (4) crop to 16:9, mirror, apply filter
//  (5) recording: send the frame to the recording worker; stopped: drop the sender
//  (6) publish the frame for the UI and wake it
fn camera_thread(shared: Shared, proxy: EventLoopProxy<()>, ctl: Arc<Ctl>) {
    let mut idx = ctl.cam.load(Ordering::Relaxed) as usize;
    let mut light = ctl.light.load(Ordering::Relaxed);
    let mut cam = open_cam(idx, light);
    if cam.is_none() && idx != 0 {
        idx = 0;
        cam = open_cam(idx, light);
    }
    if cam.is_none() {
        post(&ctl, "camera open failed - check Settings > Privacy > Camera");
        let _ = proxy.send_event(());
    }
    let mut tx: Option<mpsc::SyncSender<(Instant, Arc<Shot>)>> = None;
    let mut bad = 0u32; // frame read errors in a row
    let mut dbad = 0u32; // decode errors in a row
    loop {
        // (1) requests from the UI: bit 1 = next camera, bit 2 = toggle best/light quality
        let req = ctl.req.load(Ordering::Relaxed);
        if req != 0 {
            ctl.req.store(0, Ordering::Relaxed);
            if ctl.recording.load(Ordering::Relaxed) || tx.is_some() {
                post(&ctl, "stop recording first");
            } else {
                let (mut ni, mut nlight, mut n) = (idx, light, 1usize);
                if req & 1 != 0 {
                    n = nokhwa::query(ApiBackend::Auto).map(|v| v.len()).unwrap_or(1).max(1);
                    ni = (idx + 1) % n;
                }
                if req & 2 != 0 {
                    nlight = !light;
                }
                if (ni, nlight) == (idx, light) {
                    post(&ctl, "only one camera found");
                } else {
                    if let Some(c) = cam.as_mut() {
                        let _ = c.stop_stream();
                    }
                    drop(cam.take()); // release the device before opening the next one
                    match open_cam(ni, nlight) {
                        Some(c) => {
                            cam = Some(c);
                            idx = ni;
                            light = nlight;
                            ctl.cam.store(idx as u8, Ordering::Relaxed);
                            ctl.light.store(light, Ordering::Relaxed);
                            save_settings(&ctl);
                            if req & 1 != 0 {
                                post(&ctl, &format!("camera {} of {}", idx + 1, n));
                            } else {
                                post(&ctl, if light { "light quality 360p" } else { "best quality" });
                            }
                        }
                        None => {
                            cam = open_cam(idx, light);
                            post(&ctl, "switch failed");
                        }
                    }
                }
            }
        }

        // (2) no camera right now: stop a running recording and keep trying to reconnect
        if cam.is_none() {
            if tx.take().is_some() {
                ctl.recording.store(false, Ordering::Relaxed);
                post(&ctl, "camera lost - video saved");
            }
            if proxy.send_event(()).is_err() {
                return;
            }
            std::thread::sleep(Duration::from_millis(700));
            cam = open_cam(idx, light);
            if cam.is_some() {
                post(&ctl, "camera connected");
            }
            continue;
        }

        // (3) read + decode one frame
        let c = cam.as_mut().unwrap();
        let buf = match c.frame() {
            Ok(b) => {
                bad = 0;
                b
            }
            Err(_) => {
                bad += 1;
                if bad >= 300 {
                    cam = None;
                    bad = 0;
                    post(&ctl, "camera lost - reconnecting");
                } else {
                    std::thread::sleep(Duration::from_millis(10));
                }
                continue;
            }
        };
        let img = match buf.decode_image::<RgbFormat>() {
            Ok(i) => {
                dbad = 0;
                i
            }
            Err(_) => {
                dbad += 1;
                if dbad == 200 {
                    post(&ctl, "camera picture cannot be decoded - try Q for light quality");
                    let _ = proxy.send_event(());
                }
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
        };

        // (4) always 16:9 (crop top/bottom if the camera gave a taller picture), mirror, filter
        let (w, mut h) = (img.width(), img.height());
        let mut raw: Vec<u8> = img.into_raw();
        let nh = ((w * 9 / 16) & !1).max(2);
        if h > nh + 1 {
            let y0 = ((h - nh) / 2) as usize;
            let rowb = w as usize * 3;
            raw.truncate((y0 + nh as usize) * rowb);
            raw.drain(..y0 * rowb);
            h = nh;
        }
        // mirror once here, so preview, photo and video all look the same
        let wu = w as usize;
        for row in raw.chunks_exact_mut(wu * 3) {
            for x in 0..wu / 2 {
                let a = x * 3;
                let b = (wu - 1 - x) * 3;
                for k in 0..3 {
                    row.swap(a + k, b + k);
                }
            }
        }
        let f = ctl.filter.load(Ordering::Relaxed);
        if f != 0 {
            apply_filter(&mut raw, f);
        }
        let shot = Arc::new(Shot { w, h, raw });

        // (5) recording: the worker thread is started with the first frame, and gets every frame
        if ctl.recording.load(Ordering::Relaxed) {
            if tx.is_none() {
                ctl.busy.store(true, Ordering::Relaxed);
                tx = Some(start_recorder(ctl.clone(), w, h));
            }
            let mut dead = false;
            if let Some(t) = tx.as_ref() {
                if let Err(TrySendError::Disconnected(_)) = t.try_send((Instant::now(), shot.clone())) {
                    dead = true;
                }
            }
            if dead {
                tx = None;
            }
        } else {
            tx = None; // dropping the sender tells the worker to finish the video
        }

        // (6) publish the newest frame and wake the UI
        *shared.lock().unwrap() = Some(shot);
        if proxy.send_event(()).is_err() {
            break;
        }
    }
}

// UI state (winit). t0 = start time (for "camera ready in X ms"); xmap/xkey = cached column
// map for fast preview scaling; cur/hover/press = mouse and button animation state.
struct App {
    proxy: EventLoopProxy<()>,
    shared: Shared,
    ctl: Arc<Ctl>,
    t0: Instant,
    first: bool,
    rec_ui: bool,
    rec_start: Instant,
    rec_note: Option<String>,
    last_rec: String,
    help: bool,
    shift: bool,
    xmap: Vec<usize>,
    xkey: (usize, usize),
    cur: (f64, f64),
    hover: bool,
    press: Option<(u8, Instant)>,
    window: Option<Rc<Window>>,
    surface: Option<softbuffer::Surface<Rc<Window>, Rc<Window>>>,
}

impl App {
    fn title(&self, t: &str) {
        if let Some(w) = &self.window {
            w.set_title(t);
        }
    }

    fn refresh_rec(&mut self) {
        if !self.rec_ui {
            return;
        }
        let s = self.rec_start.elapsed().as_secs();
        let mut t = format!("FastCam - REC {:02}:{:02}", s / 60, s % 60);
        if self.ctl.mute.load(Ordering::Relaxed) {
            t.push_str(" - MIC OFF");
        }
        if let Some(n) = &self.rec_note {
            t.push_str(&format!(" ({n})"));
        }
        if t != self.last_rec {
            self.title(&t);
            self.last_rec = t;
        }
    }

    fn take_photo(&self) {
        let shot = match self.shared.lock().unwrap().clone() {
            Some(s) => s,
            None => return,
        };
        let ctl = self.ctl.clone();
        std::thread::spawn(move || {
            // reserve a unique file name, then write the JPEG (quality 92; the default 75 looks soft)
            let ok = match unique_path(&user_dir("Pictures"), "photo", "jpg") {
                Some(path) => std::fs::File::create(&path)
                    .map_err(|e| e.to_string())
                    .and_then(|f| {
                        JpegEncoder::new_with_quality(std::io::BufWriter::new(f), 92)
                            .write_image(&shot.raw, shot.w, shot.h, ExtendedColorType::Rgb8)
                            .map_err(|e| e.to_string())
                    })
                    .is_ok(),
                None => false,
            };
            post(&ctl, if ok { "photo saved in Pictures\\FastCam" } else { "photo save failed" });
        });
    }

    fn toggle_video(&mut self) {
        let on = !self.ctl.recording.load(Ordering::Relaxed);
        self.ctl.recording.store(on, Ordering::Relaxed);
        self.rec_ui = on;
        if on {
            self.rec_start = Instant::now();
            self.rec_note = None;
            self.ctl.mute.store(false, Ordering::Relaxed);
            self.last_rec.clear();
            self.refresh_rec();
        } else {
            self.title("FastCam - video saved in Videos\\FastCam");
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn toggle_mute(&mut self) {
        if self.rec_ui {
            let m = !self.ctl.mute.load(Ordering::Relaxed);
            self.ctl.mute.store(m, Ordering::Relaxed);
            self.last_rec.clear();
            self.refresh_rec();
        }
    }

    fn toggle_help(&mut self) {
        self.help = !self.help;
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn open_dir(&self, sub: &str) {
        let dir = user_dir(sub);
        let _ = std::fs::create_dir_all(&dir);
        let _ = std::process::Command::new("explorer").arg(dir).spawn();
    }

    fn toggle_fs(&self, off_only: bool) {
        if let Some(w) = &self.window {
            if w.fullscreen().is_some() {
                w.set_fullscreen(None);
            } else if !off_only {
                w.set_fullscreen(Some(Fullscreen::Borderless(None)));
            }
        }
    }

    fn next_filter(&self) {
        let f = (self.ctl.filter.load(Ordering::Relaxed) + 1) % FILTERS.len() as u8;
        self.ctl.filter.store(f, Ordering::Relaxed);
        save_settings(&self.ctl);
        self.title(&format!("FastCam - filter: {}", FILTERS[f as usize]));
    }

    fn over(&self) -> Option<u8> {
        let win = self.window.as_ref()?;
        let s = win.inner_size();
        let u = ui_scale(win);
        let (p, v) = buttons(s.width as i32, s.height as i32, u);
        let (mx, my) = (self.cur.0 as i32, self.cur.1 as i32);
        let rad = (R as f32 * u) as i32;
        let hit = |c: (i32, i32)| (mx - c.0).pow(2) + (my - c.1).pow(2) <= rad * rad;
        if hit(p) {
            Some(0)
        } else if hit(v) {
            Some(1)
        } else {
            None
        }
    }

    fn click(&mut self) {
        if self.help {
            self.toggle_help();
            return;
        }
        let (mx, my) = (self.cur.0 as i32, self.cur.1 as i32);
        let u = self.window.as_ref().map(|w| ui_scale(w)).unwrap_or(1.0);
        let (hx, hy) = hint_end(u);
        if mx >= 0 && mx <= hx && my >= 0 && my <= hy {
            self.toggle_help();
            return;
        }
        match self.over() {
            Some(0) => {
                self.press = Some((0, Instant::now()));
                self.take_photo();
            }
            Some(1) => {
                self.press = Some((1, Instant::now()));
                self.toggle_video();
            }
            _ => {}
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }
}

// winit event handling: resumed = create window + start camera thread;
// user_event = new frame/status arrived from camera thread; window_event = keyboard, mouse, redraw.
impl ApplicationHandler<()> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        // decode the exe icon once for the window and the taskbar (it used to be decoded twice)
        let icon = Icon::from_resource(1, None).ok();
        let attrs = Window::default_attributes()
            .with_title("FastCam")
            .with_window_icon(icon.clone())
            .with_taskbar_icon(icon)
            .with_inner_size(LogicalSize::new(800, 450));
        let window = Rc::new(el.create_window(attrs).unwrap());
        let sf = window.scale_factor();
        let sm = (16.0 * sf).round() as u32;
        let bg = (32.0 * sf).round() as u32;
        window.set_window_icon(Icon::from_resource(1, Some(winit::dpi::PhysicalSize::new(sm, sm))).ok());
        window.set_taskbar_icon(Icon::from_resource(1, Some(winit::dpi::PhysicalSize::new(bg, bg))).ok());
        let context = softbuffer::Context::new(window.clone()).unwrap();
        let surface = softbuffer::Surface::new(&context, window.clone()).unwrap();
        self.window = Some(window);
        self.surface = Some(surface);

        let shared = self.shared.clone();
        let proxy = self.proxy.clone();
        let ctl = self.ctl.clone();
        std::thread::spawn(move || camera_thread(shared, proxy, ctl));
    }

    fn user_event(&mut self, _el: &ActiveEventLoop, _e: ()) {
        if !self.first && self.shared.lock().unwrap().is_some() {
            self.first = true;
            let ms = self.t0.elapsed().as_millis();
            self.title(&format!("FastCam - camera ready in {ms} ms"));
        }
        if self.rec_ui && !self.ctl.recording.load(Ordering::Relaxed) {
            self.rec_ui = false;
            self.title("FastCam - video failed to start");
        }
        if self.rec_ui {
            let n = self.ctl.note.lock().unwrap().take();
            if n.is_some() {
                self.rec_note = n;
                self.last_rec.clear();
            }
            self.refresh_rec();
        }
        let m = self.ctl.msg.lock().unwrap().take();
        if let Some(m) = m {
            self.title(&format!("FastCam - {m}"));
        }
        if let Some(w) = &self.window {
            w.request_redraw();
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.ctl.recording.store(false, Ordering::Relaxed);
                // hide the window at once, then give a running recording up to 15 s to be saved properly
                if let Some(w) = &self.window {
                    w.set_visible(false);
                }
                for _ in 0..300 {
                    if !self.ctl.busy.load(Ordering::Relaxed) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                el.exit();
            }
            WindowEvent::ModifiersChanged(m) => {
                self.shift = m.state().shift_key();
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.cur = (position.x, position.y);
                let h = self.over().is_some();
                if h != self.hover {
                    self.hover = h;
                    if let Some(w) = &self.window {
                        w.set_cursor(if h { CursorIcon::Pointer } else { CursorIcon::Default });
                    }
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button: MouseButton::Left, .. } => {
                self.click();
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed && !event.repeat {
                    match event.physical_key {
                        PhysicalKey::Code(KeyCode::Space) => {
                            self.press = Some((0, Instant::now()));
                            self.take_photo();
                        }
                        PhysicalKey::Code(KeyCode::KeyV) => {
                            self.press = Some((1, Instant::now()));
                            self.toggle_video();
                        }
                        PhysicalKey::Code(KeyCode::KeyM) => self.toggle_mute(),
                        PhysicalKey::Code(KeyCode::KeyO) => {
                            self.open_dir(if self.shift { "Videos" } else { "Pictures" })
                        }
                        PhysicalKey::Code(KeyCode::KeyH) | PhysicalKey::Code(KeyCode::F1) => {
                            self.toggle_help()
                        }
                        PhysicalKey::Code(KeyCode::F11) => self.toggle_fs(false),
                        PhysicalKey::Code(KeyCode::Escape) => {
                            if self.help {
                                self.toggle_help();
                            } else {
                                self.toggle_fs(true);
                            }
                        }
                        PhysicalKey::Code(KeyCode::KeyC) => {
                            self.ctl.req.fetch_or(1, Ordering::Relaxed);
                        }
                        PhysicalKey::Code(KeyCode::KeyQ) => {
                            self.ctl.req.fetch_or(2, Ordering::Relaxed);
                        }
                        PhysicalKey::Code(KeyCode::KeyB) => self.next_filter(),
                        _ => {}
                    }
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(w) = &self.window {
                    w.request_redraw();
                }
            }
            WindowEvent::RedrawRequested => {
                // REDRAW: draw order = background, camera picture (scaled, aspect kept), flash,
                // buttons, then help panel or hint.
                // grab the latest frame and release the lock right away,
                // so the camera thread is never blocked while we draw
                let shot = self.shared.lock().unwrap().clone();
                if let (Some(window), Some(surface)) = (&self.window, &mut self.surface) {
                    let size = window.inner_size();
                    if let (Some(w), Some(h)) =
                        (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                    {
                        surface.resize(w, h).unwrap();
                        let mut buffer = surface.buffer_mut().unwrap();
                        buffer.fill(0x00202020);
                        let (ww, wh) = (size.width as usize, size.height as usize);
                        let u = ui_scale(window);
                        if let Some(f) = shot.as_ref() {
                            let (fw, fh) = (f.w as usize, f.h as usize);
                            if fw > 0 && fh > 0 && f.raw.len() >= fw * fh * 3 {
                                // keep the camera's aspect ratio (black bars when needed)
                                let (dw, dh) = if ww * fh <= wh * fw {
                                    (ww, ww * fh / fw)
                                } else {
                                    (wh * fw / fh, wh)
                                };
                                let (dw, dh) = (dw.max(1), dh.max(1));
                                let (ox, oy) = ((ww - dw) / 2, (wh - dh) / 2);
                                if self.xkey != (dw, fw) {
                                    self.xmap.clear();
                                    // frame is already mirrored: byte offset of the source pixel for each column
                                    self.xmap.extend((0..dw).map(|x| (x * fw / dw) * 3));
                                    self.xkey = (dw, fw);
                                }
                                // camera picture much bigger than the window: average 2x2 source pixels
                                // (smoother, no shimmering); otherwise take the nearest pixel
                                let smooth = fw >= 2 * dw && fh >= 2 * dh;
                                for y in 0..dh {
                                    let sy = y * fh / dh;
                                    let src = &f.raw[sy * fw * 3..(sy + 1) * fw * 3];
                                    let o = (oy + y) * ww + ox;
                                    let dst = &mut buffer[o..o + dw];
                                    if smooth {
                                        let src2 = &f.raw[(sy + 1) * fw * 3..(sy + 2) * fw * 3];
                                        for (d, &sx) in dst.iter_mut().zip(self.xmap.iter()) {
                                            let a = &src[sx..sx + 6];
                                            let b = &src2[sx..sx + 6];
                                            let r = (a[0] as u32 + a[3] as u32 + b[0] as u32 + b[3] as u32) >> 2;
                                            let g = (a[1] as u32 + a[4] as u32 + b[1] as u32 + b[4] as u32) >> 2;
                                            let bl = (a[2] as u32 + a[5] as u32 + b[2] as u32 + b[5] as u32) >> 2;
                                            *d = (r << 16) | (g << 8) | bl;
                                        }
                                    } else {
                                        for (d, &sx) in dst.iter_mut().zip(self.xmap.iter()) {
                                            let p = &src[sx..sx + 3];
                                            *d = ((p[0] as u32) << 16) | ((p[1] as u32) << 8) | p[2] as u32;
                                        }
                                    }
                                }
                            }
                        }
                        let mut press = None;
                        if let Some((id, ts)) = self.press {
                            let e = ts.elapsed().as_millis() as f32;
                            if id == 0 && e < FLASH_MS {
                                let a = 0.55 * (1.0 - e / FLASH_MS);
                                for p in buffer.iter_mut() {
                                    *p = mix(*p, 0x00FFFFFF, a);
                                }
                            }
                            if e < PRESS_MS {
                                press = Some((id, e / PRESS_MS));
                            }
                        }
                        let rec = self.ctl.recording.load(Ordering::Relaxed);
                        draw_ui(&mut buffer, ww as i32, wh as i32, rec, press, u);
                        if self.help {
                            draw_help(&mut buffer, ww, wh);
                        } else {
                            draw_hint(&mut buffer, ww, wh, u);
                        }
                        buffer.present().unwrap();
                    }
                }
            }
            _ => {}
        }
    }
}

// Entry point: builds the event loop and the App, then runs it.
fn main() {
    let t0 = Instant::now();
    let settings = load_settings(); // remembered filter, quality, camera
    let el = EventLoop::<()>::with_user_event().build().unwrap();
    let mut app = App {
        proxy: el.create_proxy(),
        shared: Arc::new(Mutex::new(None)),
        ctl: Arc::new(Ctl {
            recording: AtomicBool::new(false),
            busy: AtomicBool::new(false),
            mute: Arc::new(AtomicBool::new(false)),
            req: AtomicU8::new(0),
            filter: AtomicU8::new(settings.0),
            light: AtomicBool::new(settings.1),
            cam: AtomicU8::new(settings.2),
            note: Mutex::new(None),
            msg: Mutex::new(None),
        }),
        t0,
        first: false,
        rec_ui: false,
        rec_start: t0,
        rec_note: None,
        last_rec: String::new(),
        help: false,
        shift: false,
        xmap: Vec::new(),
        xkey: (0, 0),
        cur: (0.0, 0.0),
        hover: false,
        press: None,
        window: None,
        surface: None,
    };
    el.run_app(&mut app).unwrap();
}
