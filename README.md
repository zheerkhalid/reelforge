<div align="center">

<img src="docs/assets/reelforge-icon.png" alt="ReelForge logo" width="128" height="128" />

# ReelForge

### Convert any video or audio, fast. Your graphics card does the heavy lifting.

A friendly, native video and audio converter for Windows, written in **Rust**.
Pick a file, pick one of **408 ready-made profiles**, and press **Convert**.

<p>
  <img alt="Platform" src="https://img.shields.io/badge/Windows-10%20%7C%2011-0078D6?style=for-the-badge&logo=windows11&logoColor=white" />
  <img alt="Rust" src="https://img.shields.io/badge/Rust-stable-B7410E?style=for-the-badge&logo=rust&logoColor=white" />
  <img alt="License" src="https://img.shields.io/badge/License-MIT-E02F7A?style=for-the-badge" />
  <img alt="Profiles" src="https://img.shields.io/badge/profiles-408-FF8A3D?style=for-the-badge" />
</p>

<p>
  <img alt="NVIDIA NVENC" src="https://img.shields.io/badge/NVIDIA-NVENC-76B900?style=flat-square&logo=nvidia&logoColor=white" />
  <img alt="AMD AMF" src="https://img.shields.io/badge/AMD-AMF-ED1C24?style=flat-square&logo=amd&logoColor=white" />
  <img alt="Intel Quick Sync" src="https://img.shields.io/badge/Intel-Quick%20Sync-0071C5?style=flat-square&logo=intel&logoColor=white" />
  <img alt="Powered by FFmpeg" src="https://img.shields.io/badge/engine-FFmpeg-007808?style=flat-square&logo=ffmpeg&logoColor=white" />
  <img alt="egui" src="https://img.shields.io/badge/UI-egui-4F6BFF?style=flat-square" />
  <img alt="Made by Zheer Khalid" src="https://img.shields.io/badge/made%20by-Zheer%20Khalid-E02F7A?style=flat-square" />
</p>

[**Features**](#-features) ·
[**Screenshots**](#%EF%B8%8F-screenshots) ·
[**Download**](#-download) ·
[**Profiles**](#-profile-library) ·
[**Build**](#%EF%B8%8F-build-from-source) ·
[**Developer**](#-developer)

<img src="docs/screenshots/main.png" alt="ReelForge main window" width="860" />

</div>

---

## ✨ Features

<table>
<tr>
<td width="50%" valign="top">

### ⚡ GPU acceleration
- Finds every graphics card Windows reports and **proves each hardware encoder works** with a tiny test encode, so it only offers what actually runs
- **NVIDIA NVENC**, **AMD AMF** and **Intel Quick Sync** for H.264, HEVC and AV1
- Discrete (external) cards are preferred over integrated ones
- GPU decoding as well as encoding, with automatic fallback to the processor

</td>
<td width="50%" valign="top">

### 🎯 Easy to use
- Drag and drop files, whole folders or a DVD / Blu-ray folder
- One searchable picker: *"iPhone"*, *"4K"*, *"MP3"*, *"YouTube"*…
- Plain-language profile names and descriptions
- **Expected output size** before you convert
- Recent and favourite profiles, plus your own saved profiles

</td>
</tr>
<tr>
<td valign="top">

### ✂️ Edit before converting
- Trim, crop, rotate and flip
- Brightness, contrast and saturation, deinterlace
- Volume and loudness normalising
- Text watermark with position and size
- Subtitles: burn in tracks from the file (text or picture) or load `.srt` / `.ass` / `.ssa` files, with right-to-left script support (Arabic, Kurdish Sorani)

</td>
<td valign="top">

### 🧰 Batch power tools
- Parallel conversions, with a set number of jobs at once
- **Merge** several files into one, or **split** by time
- **Remux** (change container) in seconds with no quality loss
- Pick the audio track, extract audio, take snapshots
- Live progress, speed and the exact FFmpeg command log
- *Shut down when done* for long overnight batches

</td>
</tr>
</table>

## 🖼️ Screenshots

<table>
<tr>
<td align="center"><img src="docs/screenshots/formats.png" alt="Format picker" /><br/><sub><b>Format picker</b>: 408 profiles in 20 categories, searchable</sub></td>
</tr>
<tr>
<td align="center"><img src="docs/screenshots/customize.png" alt="Customize format" /><br/><sub><b>Customize</b>: codec, size, frame rate, quality and audio, with a before/after table and the expected size</sub></td>
</tr>
</table>

## 📥 Download

Grab **`ReelForge.exe`** from the [**latest release**](https://github.com/zheerkhalid/reelforge/releases/latest). It is a single portable EXE with no installer and no runtimes.

ReelForge uses **[FFmpeg](https://ffmpeg.org)** as its conversion engine. FFmpeg is not bundled. Install it with one of these:

```powershell
winget install Gyan.FFmpeg
```

or put `ffmpeg.exe` and `ffprobe.exe` next to `ReelForge.exe` (in a `bin\` or `ffmpeg\bin\` subfolder works too).

## 📚 Profile library

| Category | Profiles | Category | Profiles |
|---|---:|---|---:|
| ⭐ Recommended | 11 | 🗄️ Lossless & Archive | 7 |
| 🎬 Common Video | 40 | ⚡ Remux (no quality loss) | 12 |
| 🎵 Common Audio | 43 | 👓 3D Video (side-by-side, anaglyph) | 21 |
| 📺 HD & 4K / 8K | 65 | 💿 DVD, VCD & Blu-ray | 11 |
| 🌐 Social & Web | 38 | 📼 DV & Interlaced | 7 |
| 📱 Apple | 23 | 🖥️ Windows & Office | 7 |
| 📱 Android | 29 | 📟 Older Phones & Players | 10 |
| 📺 TV & Media Players | 26 | 🕸️ Flash & Legacy Web | 5 |
| 🎮 Game Consoles | 13 | 🎧 Audio for Devices | 12 |
| ✂️ Editing Software (ProRes, DNxHR) | 18 | 📷 Cameras & Drones | 10 |

Formats include MP4, MKV, MOV, WebM, AVI, WMV, MPEG-2 / DVD, 3GP, GIF, ProRes, DNxHR, and H.264, HEVC (8- and 10-bit), AV1 and VP9 video; MP3, AAC, FLAC, Opus, WAV, ALAC and more for audio. Any profile can be customized and saved to **My Profiles**.

## 🏗️ Build from source

Requires Rust (stable, MSVC toolchain) on Windows.

```powershell
git clone https://github.com/zheerkhalid/reelforge
cd reelforge
cargo build --release
# -> target\release\ReelForge.exe
```

The icon is drawn in code at build time (`build.rs`, using tiny-skia) and embedded with version info. The C runtime is linked statically, so the EXE runs on a clean Windows install.

`ReelForge.exe --selftest <folder>` runs an end-to-end conversion check across the main profiles (CPU and GPU) with FFmpeg.

```
src/
  main.rs       entry point, window, command line
  app.rs        the whole UI: file list, preview, picker, customize, edit, options
  engine.rs     probing (ffprobe), FFmpeg command building, job queue and progress
  gpu.rs        graphics-card detection and hardware-encoder verification
  profiles.rs   the profile library, generated in code
  settings.rs   JSON settings in %APPDATA%\ReelForge
  text.rs       right-to-left text shaping for file names and subtitles
  selftest.rs   end-to-end conversion test
```

## 👨‍💻 Developer

<table>
<tr>
<td align="center" width="140">
  <a href="https://zheer.dev">
    <img src="docs/assets/zheer-logo.png" alt="Zheer Khalid logo" width="110" />
  </a>
</td>
<td>

**Designed and developed by [Zheer Khalid](https://zheer.dev)**

🌐 Website: **[zheer.dev](https://zheer.dev)**  
🐙 GitHub: **[@zheerkhalid](https://github.com/zheerkhalid)**  
🧩 Also by Zheer: **[SnapForge](https://github.com/zheerkhalid/snapforge)**, a fast screen capture and recording tool

If ReelForge saves you time, a ⭐ on the repo is appreciated.

</td>
</tr>
</table>

## 📄 License

MIT © 2026 [Zheer Khalid](https://zheer.dev). See [`LICENSE`](LICENSE).

FFmpeg is a separate project under its own (LGPL/GPL) license and is not distributed with ReelForge.

<div align="center">
<sub>Built with 🦀 Rust and ❤️ by <a href="https://zheer.dev">zheer.dev</a></sub>
</div>
