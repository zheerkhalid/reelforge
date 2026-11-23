//! Conversion engine: media probing, FFmpeg argument building, and jobs that
//! run in the background with progress, cancel and automatic CPU fallback.

use crate::gpu::{HwEncoder, Vendor};
use crate::profiles::{ACodec, Fit, Profile, Quality, Stereo, VCodec};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Instant;

// ---------------------------------------------------------------- tools

pub fn hidden_command<S: AsRef<std::ffi::OsStr>>(program: S) -> Command {
    #[allow(unused_mut)]
    let mut c = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    }
    c
}

fn find_tool(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) { format!("{}.exe", name) } else { name.to_string() };
    if let Ok(me) = std::env::current_exe() {
        if let Some(dir) = me.parent() {
            for cand in [dir.join(&exe), dir.join("ffmpeg").join("bin").join(&exe), dir.join("bin").join(&exe)] {
                if cand.is_file() {
                    return Some(cand);
                }
            }
        }
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let cand = dir.join(&exe);
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    #[cfg(windows)]
    if let Some(local) = dirs::data_local_dir() {
        let cand = local.join("Microsoft").join("WinGet").join("Links").join(&exe);
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

pub fn ffmpeg_path() -> Option<PathBuf> {
    static P: OnceLock<Option<PathBuf>> = OnceLock::new();
    P.get_or_init(|| find_tool("ffmpeg")).clone()
}

pub fn ffprobe_path() -> Option<PathBuf> {
    static P: OnceLock<Option<PathBuf>> = OnceLock::new();
    P.get_or_init(|| find_tool("ffprobe")).clone()
}

// ---------------------------------------------------------------- media info

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Track {
    pub codec: String,
    pub lang: String,
    pub title: String,
    pub channels: u32,
    pub text_based: bool,
    #[serde(default)]
    pub sample_rate: u32,
    /// bits per second, 0 when unknown.
    #[serde(default)]
    pub bitrate: u64,
}

impl Track {
    pub fn label(&self, n: usize) -> String {
        let mut s = format!("#{} {}", n + 1, self.codec);
        if !self.lang.is_empty() {
            s.push_str(&format!(" [{}]", self.lang));
        }
        if self.channels > 0 {
            s.push_str(&match self.channels {
                1 => " mono".to_string(),
                2 => " stereo".to_string(),
                6 => " 5.1".to_string(),
                8 => " 7.1".to_string(),
                c => format!(" {}ch", c),
            });
        }
        if !self.title.is_empty() {
            s.push_str(&format!(" – {}", self.title));
        }
        s
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MediaInfo {
    pub ok: bool,
    pub error: String,
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub vcodec: String,
    pub acodec: String,
    pub has_video: bool,
    pub audio: Vec<Track>,
    pub subs: Vec<Track>,
    pub size: u64,
    pub bitrate: u64,
    /// Video stream bitrate in bits per second (0 = unknown).
    pub vbitrate: u64,
}

impl MediaInfo {
    /// Video bitrate, worked out from the total when the file doesn't state it.
    pub fn video_bps(&self) -> u64 {
        if self.vbitrate > 0 {
            return self.vbitrate;
        }
        if !self.has_video {
            return 0;
        }
        let total = if self.bitrate > 0 {
            self.bitrate
        } else if self.duration > 0.0 {
            (self.size as f64 * 8.0 / self.duration) as u64
        } else {
            0
        };
        let audio: u64 = self.audio.iter().map(|a| if a.bitrate > 0 { a.bitrate } else { 128_000 }).sum();
        total.saturating_sub(audio)
    }
}

fn num(v: &serde_json::Value) -> u64 {
    v.as_str().and_then(|s| s.parse().ok()).or_else(|| v.as_u64()).unwrap_or(0)
}

fn parse_rate(s: &str) -> f32 {
    let mut it = s.split('/');
    let a: f32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0.0);
    let b: f32 = it.next().and_then(|x| x.parse().ok()).unwrap_or(1.0);
    if b > 0.0 {
        a / b
    } else {
        0.0
    }
}

pub fn probe(path: &Path) -> MediaInfo {
    let mut info = MediaInfo { size: std::fs::metadata(path).map(|m| m.len()).unwrap_or(0), ..Default::default() };
    let Some(fp) = ffprobe_path() else {
        info.error = "FFprobe not found".into();
        return info;
    };
    let out = hidden_command(fp).args(["-v", "error", "-show_format", "-show_streams", "-of", "json"]).arg(path).output();
    let Ok(out) = out else {
        info.error = "Could not run FFprobe".into();
        return info;
    };
    let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) else {
        info.error = "Not a media file".into();
        return info;
    };
    let fmt = &v["format"];
    info.duration = fmt["duration"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
    info.bitrate = fmt["bit_rate"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0);
    if let Some(streams) = v["streams"].as_array() {
        for st in streams {
            let kind = st["codec_type"].as_str().unwrap_or("");
            let codec = st["codec_name"].as_str().unwrap_or("?").to_string();
            let lang = st["tags"]["language"].as_str().unwrap_or("").to_string();
            let title = st["tags"]["title"].as_str().unwrap_or("").to_string();
            let attached = st["disposition"]["attached_pic"].as_i64().unwrap_or(0) == 1;
            match kind {
                "video" if !attached && !info.has_video => {
                    info.has_video = true;
                    info.vcodec = codec;
                    info.width = st["width"].as_u64().unwrap_or(0) as u32;
                    info.height = st["height"].as_u64().unwrap_or(0) as u32;
                    info.vbitrate = num(&st["bit_rate"]);
                    info.fps = parse_rate(st["avg_frame_rate"].as_str().unwrap_or("0/1"));
                    if info.fps <= 0.0 || info.fps > 1000.0 {
                        info.fps = parse_rate(st["r_frame_rate"].as_str().unwrap_or("0/1"));
                    }
                    if info.duration <= 0.0 {
                        info.duration = st["duration"].as_str().and_then(|s| s.parse().ok()).unwrap_or(0.0);
                    }
                }
                "audio" => {
                    if info.acodec.is_empty() {
                        info.acodec = codec.clone();
                    }
                    info.audio.push(Track {
                        codec,
                        lang,
                        title,
                        channels: st["channels"].as_u64().unwrap_or(0) as u32,
                        text_based: false,
                        sample_rate: num(&st["sample_rate"]) as u32,
                        bitrate: num(&st["bit_rate"]),
                    });
                }
                "subtitle" => {
                    let text = matches!(codec.as_str(), "subrip" | "ass" | "ssa" | "mov_text" | "webvtt" | "text");
                    info.subs.push(Track { codec, lang, title, channels: 0, text_based: text, sample_rate: 0, bitrate: 0 });
                }
                _ => {}
            }
        }
    }
    info.ok = info.has_video || !info.audio.is_empty();
    if !info.ok {
        info.error = "No video or audio found".into();
    }
    info
}

// ---------------------------------------------------------------- edits

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Edits {
    pub trim_start: f64,
    /// 0 = until the end.
    pub trim_end: f64,
    /// left, top, right, bottom in pixels.
    pub crop: [u32; 4],
    pub rotate: u32,
    pub flip_h: bool,
    pub flip_v: bool,
    pub deinterlace: bool,
    pub brightness: f32,
    pub contrast: f32,
    pub saturation: f32,
    /// Percent.
    pub volume: f32,
    pub normalize: bool,
    pub watermark: String,
    /// 0 top-left, 1 top-right, 2 bottom-left, 3 bottom-right, 4 centre.
    pub watermark_pos: u8,
    pub watermark_size: u32,
}

impl Default for Edits {
    fn default() -> Self {
        Edits {
            trim_start: 0.0,
            trim_end: 0.0,
            crop: [0; 4],
            rotate: 0,
            flip_h: false,
            flip_v: false,
            deinterlace: false,
            brightness: 0.0,
            contrast: 1.0,
            saturation: 1.0,
            volume: 100.0,
            normalize: false,
            watermark: String::new(),
            watermark_pos: 3,
            watermark_size: 28,
        }
    }
}

impl Edits {
    pub fn is_default(&self) -> bool {
        *self == Edits::default()
    }
    pub fn effective_duration(&self, full: f64) -> f64 {
        let end = if self.trim_end > 0.0 { self.trim_end.min(full.max(self.trim_end)) } else { full };
        (end - self.trim_start).max(0.0)
    }
    /// Picture filters (used for conversion and for the preview).
    pub fn picture_filters(&self) -> Vec<String> {
        let mut f = Vec::new();
        if self.deinterlace {
            f.push("yadif".to_string());
        }
        let [l, t, r, b] = self.crop;
        if l + t + r + b > 0 {
            f.push(format!("crop=iw-{}:ih-{}:{}:{}", l + r, t + b, l, t));
        }
        match self.rotate {
            90 => f.push("transpose=1".into()),
            180 => f.push("hflip,vflip".into()),
            270 => f.push("transpose=2".into()),
            _ => {}
        }
        if self.flip_h {
            f.push("hflip".into());
        }
        if self.flip_v {
            f.push("vflip".into());
        }
        if self.brightness != 0.0 || self.contrast != 1.0 || self.saturation != 1.0 {
            f.push(format!("eq=brightness={:.3}:contrast={:.3}:saturation={:.3}", self.brightness, self.contrast, self.saturation));
        }
        f
    }
    pub fn watermark_filter(&self) -> Option<String> {
        if self.watermark.trim().is_empty() {
            return None;
        }
        let text = self.watermark.replace('\\', "\\\\\\\\").replace(':', "\\:").replace('\'', "\u{2019}").replace('%', "\\%");
        let (x, y) = match self.watermark_pos {
            0 => ("24", "24"),
            1 => ("w-tw-24", "24"),
            2 => ("24", "h-th-24"),
            4 => ("(w-tw)/2", "(h-th)/2"),
            _ => ("w-tw-24", "h-th-24"),
        };
        let font = if cfg!(windows) { ":fontfile='C\\:/Windows/Fonts/arial.ttf'" } else { "" };
        Some(format!("drawtext=text='{}'{}:fontsize={}:fontcolor=white@0.85:shadowcolor=black@0.6:shadowx=2:shadowy=2:x={}:y={}", text, font, self.watermark_size, x, y))
    }
}

// ---------------------------------------------------------------- jobs

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum SubChoice {
    None,
    Burn(usize),
    External(PathBuf),
}

#[derive(Clone, Debug)]
pub struct JobInput {
    pub path: PathBuf,
    pub info: MediaInfo,
    pub edits: Edits,
    pub audio_track: usize,
    pub subtitle: SubChoice,
    /// Picture subtitles: only lines the disc marks as forced.
    pub forced_only: bool,
    /// Text subtitle font size (0 = the file's own style).
    pub sub_size: u32,
}

#[derive(Clone, Debug)]
pub struct JobSpec {
    pub inputs: Vec<JobInput>,
    pub profile: Profile,
    pub output: PathBuf,
    pub hw_encoder: Option<HwEncoder>,
    pub hw_decode: Option<Vendor>,
    pub threads: u32,
    pub split_minutes: f64,
}

impl JobSpec {
    pub fn total_duration(&self) -> f64 {
        self.inputs.iter().map(|i| i.edits.effective_duration(i.info.duration)).sum()
    }
}

fn filter_path(p: &Path) -> String {
    // Escape a path for use inside a filtergraph option value.
    let s = p.to_string_lossy().replace('\\', "/");
    s.replace(':', "\\:").replace('\'', "\\'").replace('[', "\\[").replace(']', "\\]").replace(',', "\\,")
}

/// Estimated video bitrate when a codec needs one (kbps).
pub fn estimate_kbps(height: u32, q: Quality) -> u32 {
    let h = if height == 0 { 1080 } else { height };
    let base = match h {
        0..=300 => 400,
        301..=400 => 900,
        401..=600 => 1500,
        601..=800 => 3000,
        801..=1200 => 6000,
        1201..=1600 => 12000,
        1601..=2400 => 25000,
        _ => 60000,
    } as f32;
    let k = match q {
        Quality::Best => 1.6,
        Quality::High => 1.0,
        Quality::Medium => 0.7,
        Quality::Low => 0.45,
        Quality::Bitrate => 1.0,
    };
    (base * k) as u32
}

fn out_height(p: &Profile, src: &MediaInfo) -> u32 {
    if p.height > 0 {
        p.height
    } else if p.width > 0 && src.width > 0 {
        (p.width as f64 * src.height as f64 / src.width as f64) as u32
    } else {
        src.height
    }
}

fn scale_filters(p: &Profile) -> Vec<String> {
    let (w, h) = (p.width, p.height);
    if w == 0 && h == 0 {
        return vec![];
    }
    match p.fit {
        Fit::Height => {
            if h > 0 {
                vec![format!("scale=-2:{}:flags=lanczos", h)]
            } else {
                vec![format!("scale={}:-2:flags=lanczos", w)]
            }
        }
        Fit::Pad => vec![format!("scale={w}:{h}:force_original_aspect_ratio=decrease:flags=lanczos,pad={w}:{h}:(ow-iw)/2:(oh-ih)/2:color=black,setsar=1")],
        Fit::Fill => vec![format!("scale={w}:{h}:force_original_aspect_ratio=increase:flags=lanczos,crop={w}:{h},setsar=1")],
    }
}

fn stereo_filter(s: Stereo) -> Option<&'static str> {
    match s {
        Stereo::None => None,
        Stereo::AnaglyphFromSbs => Some("stereo3d=sbsl:arcd"),
        Stereo::AnaglyphFromTb => Some("stereo3d=abl:arcd"),
        Stereo::SbsToTb => Some("stereo3d=sbsl:abl"),
        Stereo::TbToSbs => Some("stereo3d=abl:sbsl"),
        Stereo::SbsHalf => Some("stereo3d=sbsl:sbs2l"),
    }
}

fn has_target(p: &Profile) -> bool {
    p.extra.iter().any(|e| e == "-target")
}

/// Character set of a subtitle file that isn't UTF-8 (old .srt files are
/// often saved in a Windows code page; Arabic-script ones in 1256).
fn subtitle_charenc(path: &Path) -> Option<&'static str> {
    let bytes = std::fs::read(path).ok()?;
    let body = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF][..]).unwrap_or(&bytes);
    if body.starts_with(&[0xFF, 0xFE]) {
        return Some("UTF-16LE");
    }
    if body.starts_with(&[0xFE, 0xFF]) {
        return Some("UTF-16BE");
    }
    if std::str::from_utf8(body).is_ok() {
        return None;
    }
    let high = body.iter().filter(|b| **b >= 0x80).count();
    let letters = body.iter().filter(|b| b.is_ascii_alphabetic()).count();
    // Mostly non-Latin text: Arabic script code page.
    if high > letters {
        Some("CP1256")
    } else {
        Some("CP1252")
    }
}

/// The subtitle choice is a picture subtitle track (Blu-ray PGS, DVD, DVB).
pub fn is_picture_sub(inp: &JobInput) -> Option<usize> {
    match inp.subtitle {
        SubChoice::Burn(i) if inp.info.subs.get(i).map(|s| !s.text_based).unwrap_or(false) => Some(i),
        _ => None,
    }
}

/// libass filter for text subtitles (a track inside the file, or a file).
/// `shift` = seconds the input was seeked by, so timings still line up.
pub fn text_subtitle_filters(path: &Path, sub: &SubChoice, subs: &[Track], shift: f64, size: u32) -> Vec<String> {
    let style = if size > 0 { format!(":force_style='Fontsize={}'", size) } else { String::new() };
    let sf = match sub {
        SubChoice::Burn(i) if subs.get(*i).map(|s| s.text_based).unwrap_or(false) => {
            format!("subtitles=filename='{}':si={}{}", filter_path(path), i, style)
        }
        SubChoice::External(file) => {
            let enc = subtitle_charenc(file).map(|e| format!(":charenc={}", e)).unwrap_or_default();
            format!("subtitles=filename='{}'{}{}", filter_path(file), enc, style)
        }
        _ => return vec![],
    };
    if shift > 0.0 {
        vec![format!("setpts=PTS+{:.3}/TB", shift), sf, "setpts=PTS-STARTPTS".into()]
    } else {
        vec![sf]
    }
}

/// A typed or written time: "HH:MM:SS,mmm", "HH:MM:SS.mmm", "H:MM:SS.cc",
/// "MM:SS.mmm" or plain seconds ("90", "12.5").
pub fn parse_clock(s: &str) -> Option<f64> {
    let s = s.trim().replace(',', ".");
    let parts: Vec<&str> = s.split(':').collect();
    let nums: Vec<f64> = parts.iter().map(|p| p.trim().parse::<f64>()).collect::<Result<_, _>>().ok()?;
    if nums.iter().any(|n| !n.is_finite() || *n < 0.0) {
        return None;
    }
    match nums.as_slice() {
        [h, m, sec] => Some(h * 3600.0 + m * 60.0 + sec),
        [m, sec] => Some(m * 60.0 + sec),
        [sec] => Some(*sec),
        _ => None,
    }
}

/// When the first subtitle line appears (seconds), so the preview can show it.
pub fn first_subtitle_time(media: &Path, sub: &SubChoice) -> Option<f64> {
    match sub {
        SubChoice::None => None,
        SubChoice::External(file) => {
            let bytes = std::fs::read(file).ok()?;
            let text = String::from_utf8_lossy(&bytes);
            for line in text.lines() {
                if let Some((start, _)) = line.split_once("-->") {
                    if let Some(t) = parse_clock(start) {
                        return Some(t);
                    }
                }
                if let Some(rest) = line.strip_prefix("Dialogue:") {
                    if let Some(t) = rest.split(',').nth(1).and_then(parse_clock) {
                        return Some(t);
                    }
                }
            }
            None
        }
        SubChoice::Burn(i) => {
            let fp = ffprobe_path()?;
            let out = hidden_command(fp)
                .args(["-v", "error", "-select_streams", &format!("s:{}", i), "-show_entries", "packet=pts_time,duration_time", "-of", "csv=p=0", "-read_intervals", "%+#20"])
                .arg(media)
                .output()
                .ok()?;
            // Picture subtitles also send empty "clear" packets; the first one is the line itself.
            String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.split(',').next().and_then(|x| x.trim().parse::<f64>().ok())).next()
        }
    }
}

/// Overlay that draws a picture subtitle track on the video (graph prefix).
pub fn picture_sub_overlay(i: usize) -> String {
    format!("[0:v:0][0:s:{}]overlay=x=(W-w)/2:y=H-h:eof_action=pass:format=auto", i)
}

/// Full video filter chain for one input.
fn video_chain(inp: &JobInput, p: &Profile, for_merge: bool) -> Vec<String> {
    let mut f = inp.edits.picture_filters();
    f.extend(text_subtitle_filters(&inp.path, &inp.subtitle, &inp.info.subs, inp.edits.trim_start, inp.sub_size));
    if let Some(s) = stereo_filter(p.stereo) {
        f.push(s.into());
    }
    if !has_target(p) {
        f.extend(scale_filters(p));
        if p.fps > 0.0 {
            f.push(format!("fps={}", crate::profiles::trim_float(p.fps)));
        }
    }
    if let Some(w) = inp.edits.watermark_filter() {
        f.push(w);
    }
    if p.vcodec == VCodec::Gif && p.ext == "gif" && !for_merge {
        f.push("split[a][b];[a]palettegen=stats_mode=diff[p];[b][p]paletteuse=dither=bayer:bayer_scale=4".into());
    }
    f
}

fn needs_yuv420(v: VCodec) -> bool {
    matches!(
        v,
        VCodec::H264
            | VCodec::H265
            | VCodec::Av1
            | VCodec::Vp9
            | VCodec::Vp8
            | VCodec::Mpeg4
            | VCodec::Xvid
            | VCodec::Mpeg2
            | VCodec::Mpeg1
            | VCodec::Wmv2
            | VCodec::H263
            | VCodec::Theora
            | VCodec::Flv1
    )
}

/// Encoder options for the video stream.
/// `target_kbps`: the bitrate the size estimate promised. When known, every
/// encoder (graphics card or processor) is told to aim for it, so the file
/// comes out the size that was shown.
fn video_codec_args(p: &Profile, hw: Option<&HwEncoder>, src_h: u32, threads: u32, target_kbps: Option<u32>) -> Vec<String> {
    let mut a: Vec<String> = Vec::new();
    let q = p.quality;
    let bitrate = if q == Quality::Bitrate { p.vkbps.max(100) } else { target_kbps.filter(|k| *k >= 50).unwrap_or(0) };
    let push = |a: &mut Vec<String>, xs: &[&str]| a.extend(xs.iter().map(|s| s.to_string()));
    let qi = |vals: [i32; 4]| -> String {
        match q {
            Quality::Best => vals[0],
            Quality::High | Quality::Bitrate => vals[1],
            Quality::Medium => vals[2],
            Quality::Low => vals[3],
        }
        .to_string()
    };
    let rate_args = |a: &mut Vec<String>| {
        a.extend(["-b:v".into(), format!("{}k", bitrate), "-maxrate".into(), format!("{}k", bitrate * 3 / 2), "-bufsize".into(), format!("{}k", bitrate * 2)]);
    };

    if let Some(e) = hw {
        push(&mut a, &["-c:v", &e.name]);
        match e.vendor {
            Vendor::Nvidia => {
                push(&mut a, &["-preset", "p5", "-tune", "hq"]);
                if bitrate > 0 {
                    push(&mut a, &["-rc", "vbr"]);
                    rate_args(&mut a);
                } else {
                    push(&mut a, &["-rc", "vbr", "-cq", &qi([18, 21, 25, 30]), "-b:v", "0"]);
                }
                push(&mut a, &["-pix_fmt", if p.ten_bit { "p010le" } else { "yuv420p" }]);
            }
            Vendor::Intel => {
                push(&mut a, &["-preset", "medium"]);
                if bitrate > 0 {
                    rate_args(&mut a);
                } else {
                    push(&mut a, &["-global_quality", &qi([18, 21, 25, 30])]);
                }
                push(&mut a, &["-pix_fmt", if p.ten_bit { "p010le" } else { "nv12" }]);
            }
            Vendor::Amd | Vendor::Other => {
                push(&mut a, &["-quality", if q == Quality::Best { "quality" } else { "balanced" }]);
                if bitrate > 0 {
                    push(&mut a, &["-rc", "vbr_peak"]);
                    rate_args(&mut a);
                } else {
                    let v = qi([18, 21, 25, 30]);
                    push(&mut a, &["-rc", "cqp", "-qp_i", &v, "-qp_p", &v]);
                }
                push(&mut a, &["-pix_fmt", if p.ten_bit { "p010le" } else { "yuv420p" }]);
            }
        }
    } else {
        let enc = p.vcodec.cpu_encoder();
        push(&mut a, &["-c:v", enc]);
        match p.vcodec {
            VCodec::H264 => {
                push(
                    &mut a,
                    &[
                        "-preset",
                        match q {
                            Quality::Best => "slow",
                            Quality::High => "medium",
                            _ => "faster",
                        },
                    ],
                );
                if bitrate > 0 {
                    rate_args(&mut a)
                } else {
                    push(&mut a, &["-crf", &qi([17, 20, 23, 28])])
                }
            }
            VCodec::H265 => {
                push(
                    &mut a,
                    &[
                        "-preset",
                        match q {
                            Quality::Best => "slow",
                            Quality::High => "medium",
                            _ => "fast",
                        },
                    ],
                );
                if bitrate > 0 {
                    rate_args(&mut a)
                } else {
                    push(&mut a, &["-crf", &qi([18, 22, 26, 30])])
                }
            }
            VCodec::Av1 => {
                push(
                    &mut a,
                    &[
                        "-preset",
                        match q {
                            Quality::Best => "4",
                            Quality::High => "6",
                            _ => "8",
                        },
                    ],
                );
                // SVT-AV1 takes a plain target bitrate (a ceiling only works with CRF).
                if bitrate > 0 {
                    a.extend(["-b:v".into(), format!("{}k", bitrate)]);
                } else {
                    push(&mut a, &["-crf", &qi([24, 30, 35, 42])])
                }
            }
            VCodec::Vp9 => {
                push(
                    &mut a,
                    &[
                        "-row-mt",
                        "1",
                        "-deadline",
                        "good",
                        "-cpu-used",
                        match q {
                            Quality::Best => "1",
                            Quality::High => "2",
                            _ => "4",
                        },
                    ],
                );
                if bitrate > 0 {
                    rate_args(&mut a)
                } else {
                    push(&mut a, &["-crf", &qi([24, 31, 36, 42]), "-b:v", "0"])
                }
            }
            VCodec::Vp8 | VCodec::Wmv2 => {
                let k = if bitrate > 0 { bitrate } else { estimate_kbps(src_h, q) };
                a.extend(["-b:v".into(), format!("{}k", k)]);
            }
            VCodec::Theora => {
                if bitrate > 0 {
                    rate_args(&mut a)
                } else {
                    push(&mut a, &["-q:v", &qi([9, 7, 5, 3])])
                }
            }
            VCodec::Mpeg4 | VCodec::Xvid | VCodec::Mpeg2 | VCodec::Mpeg1 | VCodec::H263 | VCodec::Flv1 | VCodec::Mjpeg => {
                if bitrate > 0 {
                    rate_args(&mut a)
                } else {
                    push(&mut a, &["-q:v", &qi([2, 3, 5, 8])])
                }
            }
            _ => {}
        }
        if needs_yuv420(p.vcodec) {
            push(&mut a, &["-pix_fmt", if p.ten_bit { "yuv420p10le" } else { "yuv420p" }]);
        }
    }
    if p.vcodec == VCodec::H265 && matches!(p.ext.as_str(), "mp4" | "mov" | "m4v") && !p.extra.iter().any(|e| e == "-tag:v") {
        push(&mut a, &["-tag:v", "hvc1"]);
    }
    if threads > 0 && hw.is_none() {
        a.extend(["-threads".into(), threads.to_string()]);
    }
    a
}

fn audio_codec_args(p: &Profile) -> Vec<String> {
    let mut a = vec!["-c:a".to_string(), p.acodec.encoder().to_string()];
    if p.acodec == ACodec::Copy || p.acodec == ACodec::None {
        return a;
    }
    if p.akbps > 0 && !p.acodec.lossless() && !has_target(p) {
        a.extend(["-b:a".into(), format!("{}k", p.akbps)]);
    }
    let mut rate = p.sample_rate;
    let mut ch = p.channels as u32;
    if p.acodec == ACodec::AmrNb {
        rate = 8000;
        ch = 1;
    }
    if p.acodec == ACodec::Opus && ![0, 8000, 12000, 16000, 24000, 48000].contains(&rate) {
        rate = 48000;
    }
    if rate > 0 && !has_target(p) {
        a.extend(["-ar".into(), rate.to_string()]);
    }
    if ch > 0 && !has_target(p) {
        a.extend(["-ac".into(), ch.to_string()]);
    }
    a
}

fn audio_filters(e: &Edits) -> Vec<String> {
    let mut f = Vec::new();
    if (e.volume - 100.0).abs() > 0.5 {
        f.push(format!("volume={:.2}", e.volume / 100.0));
    }
    if e.normalize {
        f.push("loudnorm=I=-16:TP=-1.5:LRA=11".into());
    }
    f
}

fn s(v: &str) -> String {
    v.to_string()
}

/// Build the FFmpeg command line. `use_gpu` false forces the processor.
pub fn build_args(spec: &JobSpec, use_gpu: bool, concat_list: Option<&Path>) -> Vec<String> {
    let p = &spec.profile;
    let hw = if use_gpu { spec.hw_encoder.as_ref() } else { None };
    let mut a: Vec<String> = vec![s("-hide_banner"), s("-nostdin"), s("-y")];
    let first = &spec.inputs[0];
    let src_h = out_height(p, &first.info);
    let video_out = p.vcodec != VCodec::None && spec.inputs.iter().any(|i| i.info.has_video);
    let audio_out = p.acodec != ACodec::None && spec.inputs.iter().any(|i| !i.info.audio.is_empty());

    let add_input = |a: &mut Vec<String>, inp: &JobInput| {
        if use_gpu && video_out && p.vcodec != VCodec::Copy {
            if let Some(v) = spec.hw_decode {
                a.extend([s("-hwaccel"), s(v.hwaccel())]);
            }
        }
        if inp.edits.trim_start > 0.0 {
            a.extend([s("-ss"), format!("{:.3}", inp.edits.trim_start)]);
        }
        if inp.edits.trim_end > 0.0 && inp.edits.trim_end > inp.edits.trim_start {
            a.extend([s("-t"), format!("{:.3}", inp.edits.trim_end - inp.edits.trim_start)]);
        }
        if inp.forced_only && is_picture_sub(inp).is_some() {
            // Only the subtitles a disc marks as "forced" (foreign-language lines).
            a.extend([s("-forced_subs_only"), s("1")]);
        }
        a.push(s("-i"));
        a.push(inp.path.to_string_lossy().to_string());
    };

    let merging = spec.inputs.len() > 1;
    if let (true, Some(list)) = (merging, concat_list) {
        // Stream copy merge through the concat demuxer.
        a.extend([s("-f"), s("concat"), s("-safe"), s("0"), s("-i"), list.to_string_lossy().to_string()]);
        a.extend([s("-map"), s("0:v?"), s("-map"), s("0:a?"), s("-c"), s("copy")]);
    } else if merging {
        for inp in &spec.inputs {
            add_input(&mut a, inp);
        }
        // Common size for all parts: profile size, or the first input's.
        let (tw, th) = if p.width > 0 && p.height > 0 {
            (p.width, p.height)
        } else if p.height > 0 {
            let w = if first.info.height > 0 { (first.info.width as f64 * p.height as f64 / first.info.height as f64) as u32 } else { p.height * 16 / 9 };
            (w & !1, p.height)
        } else {
            (first.info.width.max(2) & !1, first.info.height.max(2) & !1)
        };
        let fps = if p.fps > 0.0 {
            p.fps
        } else if first.info.fps > 0.0 {
            first.info.fps
        } else {
            30.0
        };
        let mut graph = Vec::new();
        let mut concat_in = String::new();
        for (i, inp) in spec.inputs.iter().enumerate() {
            if video_out {
                let mut chain = inp.edits.picture_filters();
                chain.extend(text_subtitle_filters(&inp.path, &inp.subtitle, &inp.info.subs, inp.edits.trim_start, inp.sub_size));
                if let Some(w) = inp.edits.watermark_filter() {
                    chain.push(w);
                }
                chain.push(format!("scale={tw}:{th}:force_original_aspect_ratio=decrease,pad={tw}:{th}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps={},format=yuv420p", crate::profiles::trim_float(fps)));
                if let (true, Some(k)) = (inp.info.has_video, is_picture_sub(inp)) {
                    graph.push(format!("[{i}:v:0][{i}:s:{k}]overlay=x=(W-w)/2:y=H-h:eof_action=pass:format=auto,{}[v{i}]", chain.join(",")));
                } else if inp.info.has_video {
                    graph.push(format!("[{}:v:0]{}[v{}]", i, chain.join(","), i));
                } else {
                    graph.push(format!("color=c=black:s={}x{}:r={}:d={:.3}[v{}]", tw, th, crate::profiles::trim_float(fps), inp.edits.effective_duration(inp.info.duration), i));
                }
                concat_in.push_str(&format!("[v{}]", i));
            }
            if audio_out {
                let mut af = audio_filters(&inp.edits);
                af.push("aresample=48000".into());
                af.push("aformat=channel_layouts=stereo".into());
                if !inp.info.audio.is_empty() {
                    let t = inp.audio_track.min(inp.info.audio.len() - 1);
                    graph.push(format!("[{}:a:{}]{}[a{}]", i, t, af.join(","), i));
                } else {
                    graph.push(format!("anullsrc=r=48000:cl=stereo,atrim=duration={:.3}[a{}]", inp.edits.effective_duration(inp.info.duration), i));
                }
                concat_in.push_str(&format!("[a{}]", i));
            }
        }
        graph.push(format!(
            "{}concat=n={}:v={}:a={}{}{}",
            concat_in,
            spec.inputs.len(),
            video_out as u8,
            audio_out as u8,
            if video_out { "[vout]" } else { "" },
            if audio_out { "[aout]" } else { "" }
        ));
        let mut g = graph.join(";");
        if video_out && p.vcodec == VCodec::Gif && p.ext == "gif" {
            g = g.replace("[vout]", "[vcat]");
            g.push_str(";[vcat]split[ga][gb];[ga]palettegen=stats_mode=diff[gp];[gb][gp]paletteuse[vout]");
        }
        a.extend([s("-filter_complex"), g]);
        if video_out {
            a.extend([s("-map"), s("[vout]")]);
            let mut vp = p.clone();
            if vp.vcodec == VCodec::Copy {
                vp.vcodec = VCodec::H264;
            }
            a.extend(video_codec_args(&vp, hw, th, spec.threads, None));
        } else {
            a.push(s("-vn"));
        }
        if audio_out {
            a.extend([s("-map"), s("[aout]")]);
            let mut ap = p.clone();
            if ap.acodec == ACodec::Copy {
                ap.acodec = ACodec::Aac;
            }
            a.extend(audio_codec_args(&ap));
        } else {
            a.push(s("-an"));
        }
    } else {
        add_input(&mut a, first);
        if p.is_remux() {
            // Copy every stream the new container can hold.
            if p.ext == "mkv" {
                a.extend([s("-map"), s("0"), s("-c"), s("copy")]);
            } else {
                a.extend([s("-map"), s("0:v?"), s("-map"), s("0:a?"), s("-c"), s("copy"), s("-sn")]);
            }
            if p.acodec == ACodec::None {
                a.push(s("-an"));
            }
        } else {
            // Picture subtitles are drawn with an overlay, which needs a filter graph.
            let pic_sub = if video_out && p.vcodec != VCodec::Copy { is_picture_sub(first) } else { None };
            if video_out && pic_sub.is_none() {
                a.extend([s("-map"), s("0:v:0")]);
            }
            if audio_out {
                let t = first.audio_track.min(first.info.audio.len().saturating_sub(1));
                a.extend([s("-map"), format!("0:a:{}", t)]);
            }
            if video_out {
                if p.vcodec == VCodec::Copy {
                    a.extend([s("-c:v"), s("copy")]);
                } else {
                    let vf = video_chain(first, p, false);
                    if let Some(k) = pic_sub {
                        let mut g = picture_sub_overlay(k);
                        if !vf.is_empty() {
                            g.push(',');
                            g.push_str(&vf.join(","));
                        }
                        g.push_str("[vout]");
                        a.extend([s("-filter_complex"), g, s("-map"), s("[vout]")]);
                    } else if !vf.is_empty() {
                        a.extend([s("-vf"), vf.join(",")]);
                    }
                    let target = if has_target(p) { None } else { target_video_kbps(p, &first.info, &first.edits).map(|k| k.round() as u32) };
                    a.extend(video_codec_args(p, hw, src_h, spec.threads, target));
                }
            } else {
                a.push(s("-vn"));
            }
            if audio_out {
                if p.acodec != ACodec::Copy {
                    let af = audio_filters(&first.edits);
                    if !af.is_empty() {
                        a.extend([s("-af"), af.join(",")]);
                    }
                }
                a.extend(audio_codec_args(p));
            } else {
                a.push(s("-an"));
            }
            a.push(s("-sn"));
        }
        a.extend([s("-map_metadata"), s("0")]);
    }

    if matches!(p.ext.as_str(), "mp4" | "mov" | "m4v" | "m4a" | "m4b" | "m4r" | "3gp" | "3g2" | "f4v") && spec.split_minutes <= 0.0 {
        a.extend([s("-movflags"), s("+faststart")]);
    }
    if p.ext == "m4r" || p.ext == "m4b" {
        a.extend([s("-f"), s("ipod")]);
    }
    a.extend(p.extra.iter().cloned());

    if spec.split_minutes > 0.0 {
        let secs = spec.split_minutes * 60.0;
        if video_out && p.vcodec != VCodec::Copy {
            a.extend([s("-force_key_frames"), format!("expr:gte(t,n_forced*{:.0})", secs)]);
            // Hardware encoders only honour forced keyframes as IDR frames when asked.
            match hw.map(|e| e.vendor) {
                Some(Vendor::Nvidia) => a.extend([s("-forced-idr"), s("1")]),
                Some(Vendor::Intel) => a.extend([s("-forced_idr"), s("1")]),
                _ => {}
            }
        }
        a.extend([s("-f"), s("segment"), s("-segment_time"), format!("{:.0}", secs), s("-reset_timestamps"), s("1")]);
    }
    a.extend([s("-progress"), s("pipe:1"), s("-nostats")]);
    if spec.split_minutes > 0.0 {
        a.push(split_pattern(&spec.output).to_string_lossy().to_string());
    } else {
        a.push(spec.output.to_string_lossy().to_string());
    }
    a
}

pub fn split_pattern(out: &Path) -> PathBuf {
    let stem = out.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let ext = out.extension().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    out.with_file_name(format!("{}_part%02d.{}", stem, ext))
}

#[derive(Clone, Debug, Default)]
pub struct JobState {
    pub progress: f32,
    pub speed: f32,
    pub eta_secs: f64,
    pub out_bytes: u64,
    pub finished: bool,
    pub ok: bool,
    pub cancelled: bool,
    pub error: String,
    pub log: Vec<String>,
    pub engine: String,
    pub fell_back: bool,
    pub elapsed: f64,
    pub command: String,
}

pub struct JobHandle {
    pub state: Arc<Mutex<JobState>>,
    child: Arc<Mutex<Option<Child>>>,
    cancel: Arc<AtomicBool>,
}

impl JobHandle {
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(c) = self.child.lock().unwrap().as_mut() {
            let _ = c.kill();
        }
    }
}

fn run_once(spec: &JobSpec, use_gpu: bool, state: &Arc<Mutex<JobState>>, child_slot: &Arc<Mutex<Option<Child>>>, cancel: &Arc<AtomicBool>, ctx: &eframe::egui::Context) -> bool {
    let Some(ff) = ffmpeg_path() else {
        state.lock().unwrap().error = "FFmpeg was not found. Put ffmpeg.exe next to this program or install it.".into();
        return false;
    };
    // Stream-copy merging needs a list file.
    let mut list_file = None;
    if spec.inputs.len() > 1 && spec.profile.vcodec == VCodec::Copy {
        static LIST_NO: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!("reelforge_concat_{}_{}.txt", std::process::id(), LIST_NO.fetch_add(1, Ordering::SeqCst)));
        let body: String = spec.inputs.iter().map(|i| format!("file '{}'\n", i.path.to_string_lossy().replace('\'', "'\\''"))).collect();
        if std::fs::write(&path, body).is_ok() {
            list_file = Some(path);
        }
    }
    let args = build_args(spec, use_gpu, list_file.as_deref());
    {
        let mut st = state.lock().unwrap();
        st.command = format!("ffmpeg {}", args.iter().map(|a| if a.contains(' ') { format!("\"{}\"", a) } else { a.clone() }).collect::<Vec<_>>().join(" "));
        let c = st.command.clone();
        st.log.push(c);
        st.engine = if use_gpu {
            match (&spec.hw_encoder, spec.hw_decode) {
                (Some(e), _) if spec.profile.vcodec.gpu_capable() => {
                    format!("GPU: {}", e.vendor.short())
                }
                (_, Some(v)) => format!("GPU decode ({}), CPU encode", v.short()),
                _ => "CPU".into(),
            }
        } else {
            "CPU".into()
        };
    }
    let spawned = hidden_command(&ff).args(&args).stdout(Stdio::piped()).stderr(Stdio::piped()).stdin(Stdio::null()).spawn();
    let mut child = match spawned {
        Ok(c) => c,
        Err(e) => {
            state.lock().unwrap().error = format!("Could not start FFmpeg: {}", e);
            return false;
        }
    };
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    *child_slot.lock().unwrap() = Some(child);

    let st2 = state.clone();
    let err_thread = std::thread::spawn(move || {
        if let Some(mut e) = stderr {
            let mut buf = Vec::new();
            let _ = e.read_to_end(&mut buf);
            let text = String::from_utf8_lossy(&buf).to_string();
            let mut st = st2.lock().unwrap();
            for l in text.lines() {
                st.log.push(l.to_string());
            }
            let n = st.log.len();
            if n > 400 {
                st.log.drain(0..n - 400);
            }
        }
    });
    let total = spec.total_duration().max(0.001);
    if let Some(out) = stdout {
        for line in BufReader::new(out).lines().map_while(Result::ok) {
            let (k, v) = match line.split_once('=') {
                Some(x) => x,
                None => continue,
            };
            let mut st = state.lock().unwrap();
            match k {
                "out_time_us" | "out_time_ms" => {
                    if let Ok(us) = v.trim().parse::<f64>() {
                        let t = us / 1_000_000.0;
                        st.progress = ((t / total) as f32).clamp(0.0, 1.0);
                        if st.speed > 0.0 {
                            st.eta_secs = ((total - t) / st.speed as f64).max(0.0);
                        }
                    }
                }
                "speed" => {
                    st.speed = v.trim().trim_end_matches('x').parse().unwrap_or(0.0);
                }
                "total_size" => {
                    st.out_bytes = v.trim().parse().unwrap_or(st.out_bytes);
                }
                _ => {}
            }
            drop(st);
            ctx.request_repaint();
        }
    }
    let _ = err_thread.join();
    let status = child_slot.lock().unwrap().as_mut().and_then(|c| c.wait().ok());
    *child_slot.lock().unwrap() = None;
    if let Some(l) = list_file {
        let _ = std::fs::remove_file(l);
    }
    if cancel.load(Ordering::SeqCst) {
        return false;
    }
    let ok = status.map(|s| s.success()).unwrap_or(false);
    if !ok {
        let mut st = state.lock().unwrap();
        let last: Vec<String> = st.log.iter().rev().filter(|l| !l.trim().is_empty() && !l.starts_with("ffmpeg ")).take(3).cloned().collect();
        st.error = last.into_iter().rev().collect::<Vec<_>>().join(" · ");
        if st.error.is_empty() {
            st.error = "FFmpeg stopped with an error".into();
        }
    }
    ok
}

pub fn start_job(spec: JobSpec, ctx: eframe::egui::Context) -> JobHandle {
    let state = Arc::new(Mutex::new(JobState::default()));
    let child = Arc::new(Mutex::new(None));
    let cancel = Arc::new(AtomicBool::new(false));
    let (st, ch, ca) = (state.clone(), child.clone(), cancel.clone());
    std::thread::spawn(move || {
        let t0 = Instant::now();
        // A file that was already there belongs to the user: never delete it.
        let existed_before = spec.output.exists();
        let gpu_wanted = spec.hw_encoder.is_some() || spec.hw_decode.is_some();
        let mut ok = run_once(&spec, gpu_wanted, &st, &ch, &ca, &ctx);
        if !ok && gpu_wanted && !ca.load(Ordering::SeqCst) {
            // Graphics card refused this file: do it on the processor instead.
            {
                let mut s = st.lock().unwrap();
                s.log.push("— GPU conversion failed, retrying on the processor —".into());
                s.fell_back = true;
                s.progress = 0.0;
                s.error.clear();
            }
            ok = run_once(&spec, false, &st, &ch, &ca, &ctx);
        }
        let mut s = st.lock().unwrap();
        s.finished = true;
        s.ok = ok;
        s.cancelled = ca.load(Ordering::SeqCst);
        s.elapsed = t0.elapsed().as_secs_f64();
        if ok {
            s.progress = 1.0;
            if spec.split_minutes <= 0.0 {
                if let Ok(m) = std::fs::metadata(&spec.output) {
                    s.out_bytes = m.len();
                }
            }
        } else if should_remove_partial(existed_before, spec.split_minutes) {
            // A stopped or failed conversion leaves a broken, half-written file.
            let _ = std::fs::remove_file(&spec.output);
        }
        drop(s);
        ctx.request_repaint();
    });
    JobHandle { state, child, cancel }
}

/// Whether the half-written output of a failed or cancelled job is deleted:
/// only a file this job created itself, and not the parts of a split.
pub fn should_remove_partial(existed_before: bool, split_minutes: f64) -> bool {
    !existed_before && split_minutes <= 0.0
}

// ---------------------------------------------------------------- probing queue

/// Most files read (probed) at the same time. Adding a folder with hundreds of
/// files would otherwise start hundreds of FFprobe processes at once.
pub const PROBE_WORKERS: usize = 4;

#[derive(Default)]
struct PoolState {
    queue: VecDeque<(u64, PathBuf)>,
    workers: usize,
}

/// Reads media details in the background, a few files at a time, in the
/// order they were added. Results land in `inbox`.
#[derive(Clone, Default)]
pub struct ProbePool {
    state: Arc<Mutex<PoolState>>,
    pub inbox: Arc<Mutex<Vec<(u64, MediaInfo)>>>,
}

impl ProbePool {
    pub fn submit(&self, id: u64, path: PathBuf, ctx: &eframe::egui::Context) {
        let ctx = ctx.clone();
        self.submit_with(id, path, probe, move || ctx.request_repaint());
    }

    /// `work` reads one file; `wake` is called after each result.
    pub fn submit_with<W, K>(&self, id: u64, path: PathBuf, work: W, wake: K)
    where
        W: Fn(&Path) -> MediaInfo + Send + 'static,
        K: Fn() + Send + 'static,
    {
        let mut st = self.state.lock().unwrap();
        st.queue.push_back((id, path));
        if st.workers >= PROBE_WORKERS {
            return;
        }
        st.workers += 1;
        drop(st);
        let (state, inbox) = (self.state.clone(), self.inbox.clone());
        std::thread::spawn(move || loop {
            let next = {
                let mut st = state.lock().unwrap();
                let next = st.queue.pop_front();
                if next.is_none() {
                    // Leaving is decided under the lock, so a file queued right
                    // now either finds this worker or starts a new one.
                    st.workers -= 1;
                }
                next
            };
            let Some((id, path)) = next else { break };
            let info = work(&path);
            inbox.lock().unwrap().push((id, info));
            wake();
        });
    }

    /// Files still waiting or being read.
    #[cfg(test)]
    fn busy(&self) -> bool {
        let st = self.state.lock().unwrap();
        st.workers > 0 || !st.queue.is_empty()
    }
}

// ---------------------------------------------------------------- frames

/// Grab one frame as an RGBA image (for the preview).
pub fn grab_frame(path: &Path, t: f64, width: u32, filters: &[String]) -> Option<eframe::egui::ColorImage> {
    let ff = ffmpeg_path()?;
    let mut vf: Vec<String> = filters.iter().filter(|f| !f.starts_with("@picsub:")).cloned().collect();
    vf.push(format!("scale={}:-2", width));
    let pic_sub = filters.iter().find_map(|f| f.strip_prefix("@picsub:").and_then(|k| k.parse::<usize>().ok()));
    let mut cmd = hidden_command(ff);
    cmd.args(["-hide_banner", "-loglevel", "error"]);
    if let Some(k) = pic_sub {
        // A picture subtitle may have started a little earlier: read from
        // 15 s before so the one on screen at `t` is decoded too.
        let lead = t.clamp(0.0, 15.0);
        cmd.args(["-ss", &format!("{:.3}", (t - lead).max(0.0)), "-i"]).arg(path);
        let graph = format!("{},{}[v]", picture_sub_overlay(k), vf.join(","));
        cmd.args(["-filter_complex", &graph, "-map", "[v]", "-ss", &format!("{:.3}", lead)]);
    } else {
        cmd.args(["-ss", &format!("{:.3}", t.max(0.0)), "-i"]).arg(path);
        cmd.args(["-vf", &vf.join(",")]);
    }
    let out = cmd.args(["-frames:v", "1", "-f", "image2pipe", "-c:v", "png", "-"]).output().ok()?;
    let img = image::load_from_memory(&out.stdout).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(eframe::egui::ColorImage::from_rgba_unmultiplied(size, img.as_raw()))
}

/// Save a still picture of a moment.
pub fn save_snapshot(path: &Path, t: f64, out: &Path, deinterlace: bool) -> Result<(), String> {
    let ff = ffmpeg_path().ok_or("FFmpeg not found")?;
    let mut cmd = hidden_command(ff);
    cmd.args(["-hide_banner", "-loglevel", "error", "-y", "-ss", &format!("{:.3}", t.max(0.0)), "-i"]).arg(path);
    if deinterlace {
        cmd.args(["-vf", "yadif"]);
    }
    cmd.args(["-frames:v", "1", "-q:v", "2"]).arg(out);
    let o = cmd.output().map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).lines().last().unwrap_or("failed").to_string())
    }
}

pub fn fmt_time(secs: f64) -> String {
    let s = secs.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
    } else {
        format!("{:02}:{:02}", s / 60, s % 60)
    }
}

/// Like [`fmt_time`], with tenths of a second when the time is not whole
/// ("01:30.5"), for the trim boxes.
pub fn fmt_clock(secs: f64) -> String {
    let tenths = (secs.max(0.0) * 10.0).round() as u64;
    let whole = fmt_time((tenths / 10) as f64);
    if tenths.is_multiple_of(10) {
        whole
    } else {
        format!("{}.{}", whole, tenths % 10)
    }
}

/// Bitrates below 100 Mbps show as kbps, otherwise as Mbps;
/// file sizes use KB/MB/GB.
pub fn fmt_bytes(b: u64) -> String {
    let b = b as f64;
    if b >= 1024.0 * 1024.0 * 1024.0 {
        format!("{:.2} GB", b / 1024.0 / 1024.0 / 1024.0)
    } else if b >= 1024.0 * 1024.0 {
        format!("{:.1} MB", b / 1024.0 / 1024.0)
    } else if b >= 1024.0 {
        format!("{:.0} KB", b / 1024.0)
    } else {
        format!("{} B", b as u64)
    }
}

pub fn fmt_bitrate(bps: u64) -> String {
    if bps <= 100_000_000 {
        format!("{}kbps", bps / 1000)
    } else {
        format!("{}mbps", bps / 1_000_000)
    }
}

/// How many bits a codec needs for the same picture, relative to H.264.
fn codec_weight_name(name: &str) -> f64 {
    match name {
        "h264" => 1.0,
        "hevc" | "h265" => 0.6,
        "av1" => 0.5,
        "vp9" => 0.65,
        "vp8" => 1.1,
        "mpeg4" | "msmpeg4v3" | "xvid" | "divx" | "flv1" | "h263" | "wmv2" | "wmv3" | "vc1" => 1.5,
        "mpeg2video" | "mpeg1video" => 2.2,
        "theora" => 1.4,
        _ => 1.0,
    }
}

fn codec_weight(v: VCodec) -> f64 {
    match v {
        VCodec::H264 => 1.0,
        VCodec::H265 => 0.6,
        VCodec::Av1 => 0.5,
        VCodec::Vp9 => 0.65,
        VCodec::Vp8 => 1.1,
        VCodec::Mpeg4 | VCodec::Xvid | VCodec::Wmv2 | VCodec::H263 | VCodec::Flv1 => 1.5,
        VCodec::Mpeg2 | VCodec::Mpeg1 => 2.2,
        VCodec::Theora => 1.4,
        _ => 1.0,
    }
}

/// Picture size of the output (after crop, rotation, scaling).
pub fn out_dims(p: &Profile, info: &MediaInfo, e: &Edits) -> (u32, u32) {
    if let Some(i) = p.extra.iter().position(|x| x == "-target") {
        let t = p.extra.get(i + 1).map(|s| s.as_str()).unwrap_or("");
        let pal = t.starts_with("pal");
        return if t.ends_with("svcd") {
            (480, if pal { 576 } else { 480 })
        } else if t.ends_with("vcd") {
            (352, if pal { 288 } else { 240 })
        } else {
            (720, if pal { 576 } else { 480 })
        };
    }
    let [l, t, r, b] = e.crop;
    let mut w = info.width.saturating_sub(l + r).max(2) as f64;
    let mut h = info.height.saturating_sub(t + b).max(2) as f64;
    if e.rotate == 90 || e.rotate == 270 {
        std::mem::swap(&mut w, &mut h);
    }
    if p.vcodec == VCodec::Copy {
        return (info.width, info.height);
    }
    let even = |x: f64| ((x / 2.0).round() * 2.0) as u32;
    match (p.width, p.height, p.fit) {
        (0, 0, _) => (w as u32, h as u32),
        (_, ph, Fit::Height) if ph > 0 => (even(w * ph as f64 / h), ph),
        (pw, 0, _) => (pw, even(h * pw as f64 / w)),
        (pw, ph, _) => (pw, ph),
    }
}

/// Expected video bitrate of the output in kbps (None = can't tell).
pub fn target_video_kbps(p: &Profile, info: &MediaInfo, e: &Edits) -> Option<f64> {
    if !info.has_video {
        return Some(0.0);
    }
    let (w, h) = out_dims(p, info, e);
    let fps = if p.fps > 0.0 {
        p.fps as f64
    } else if info.fps > 0.0 {
        info.fps as f64
    } else {
        30.0
    };
    let src_fps = if info.fps > 0.0 { info.fps as f64 } else { fps };
    match p.vcodec {
        VCodec::None => Some(0.0),
        VCodec::Copy => Some(info.video_bps() as f64 / 1000.0),
        VCodec::ProRes => {
            // Apple's published 1080p30 rates (Mbps), scaled by pixels and frame rate.
            let prof = p.extra.iter().position(|x| x == "-profile:v").and_then(|i| p.extra.get(i + 1)).map(|s| s.as_str()).unwrap_or("2");
            let base = match prof {
                "0" => 45.0,
                "1" => 102.0,
                "3" => 220.0,
                "4" => 330.0,
                "5" => 500.0,
                _ => 147.0,
            };
            Some(base * 1000.0 * (w as f64 * h as f64) / (1920.0 * 1080.0) * fps / 30.0)
        }
        VCodec::DnxHr => {
            let prof = p.extra.iter().position(|x| x == "-profile:v").and_then(|i| p.extra.get(i + 1)).map(|s| s.as_str()).unwrap_or("dnxhr_hq");
            let base = match prof {
                "dnxhr_lb" => 45.0,
                "dnxhr_sq" => 145.0,
                "dnxhr_hqx" | "dnxhr_444" => 440.0,
                _ => 220.0,
            };
            Some(base * 1000.0 * (w as f64 * h as f64) / (1920.0 * 1080.0) * fps / 30.0)
        }
        VCodec::Ffv1 | VCodec::Huffyuv | VCodec::Gif | VCodec::Mjpeg => None,
        VCodec::Dv => Some(25_000.0),
        v => {
            if p.quality == Quality::Bitrate {
                return Some(p.vkbps as f64);
            }
            if has_target(p) {
                return Some(if p.extra.iter().any(|x| x.ends_with("-vcd") && !x.contains("svcd")) { 1150.0 } else { 6000.0 });
            }
            let generic = estimate_kbps(h, p.quality) as f64 * codec_weight(v) * 0.8;
            let src = info.video_bps() as f64 / 1000.0;
            if src > 0.0 && info.width > 0 && info.height > 0 {
                // Same codec at the same size never needs more than the source had.
                let pixels = (w as f64 * h as f64) / (info.width as f64 * info.height as f64);
                let q = match p.quality {
                    Quality::Best => 1.15,
                    Quality::High => 0.9,
                    Quality::Medium => 0.65,
                    _ => 0.42,
                };
                let from_src = src * pixels.clamp(0.05, 1.0) * (fps / src_fps).min(2.0) * codec_weight(v) / codec_weight_name(&info.vcodec) * q;
                Some(generic.min(from_src.max(generic * 0.25)))
            } else {
                Some(generic)
            }
        }
    }
}

/// Expected audio bitrate of the output in kbps.
pub fn target_audio_kbps(p: &Profile, info: &MediaInfo, track: usize) -> f64 {
    let src = info.audio.get(track).or_else(|| info.audio.first());
    let Some(src) = src else { return 0.0 };
    let rate = if p.sample_rate > 0 {
        p.sample_rate
    } else if src.sample_rate > 0 {
        src.sample_rate
    } else {
        48000
    } as f64;
    let ch = if p.channels > 0 { p.channels as u32 } else { src.channels.max(1) } as f64;
    match p.acodec {
        ACodec::None => 0.0,
        ACodec::Copy => {
            if src.bitrate > 0 {
                src.bitrate as f64 / 1000.0
            } else {
                192.0
            }
        }
        ACodec::Pcm16 | ACodec::Pcm16Be => rate * ch * 16.0 / 1000.0,
        ACodec::Pcm24 => rate * ch * 24.0 / 1000.0,
        ACodec::Flac | ACodec::Alac => rate * ch * 16.0 / 1000.0 * 0.6,
        ACodec::AmrNb => 12.2,
        _ if p.akbps > 0 => p.akbps as f64,
        ACodec::Opus => 96.0,
        _ => 128.0,
    }
}

/// Expected output file size in bytes.
pub fn estimate_output(p: &Profile, info: &MediaInfo, e: &Edits, track: usize) -> Option<u64> {
    let dur = e.effective_duration(info.duration);
    if dur <= 0.0 {
        return None;
    }
    let v = target_video_kbps(p, info, e)?;
    let a = target_audio_kbps(p, info, track);
    // ~2% container overhead.
    Some(((v + a) * 1000.0 / 8.0 * dur * 1.02) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles;

    fn profile(name: &str) -> Profile {
        profiles::library().into_iter().find(|p| p.name == name).unwrap_or_else(|| panic!("no profile {name}"))
    }

    fn video_info() -> MediaInfo {
        MediaInfo {
            ok: true,
            duration: 60.0,
            width: 1920,
            height: 1080,
            fps: 30.0,
            vcodec: "h264".into(),
            acodec: "aac".into(),
            has_video: true,
            audio: vec![Track { codec: "aac".into(), channels: 2, sample_rate: 48000, bitrate: 128_000, ..Default::default() }],
            size: 30_000_000,
            bitrate: 4_000_000,
            ..Default::default()
        }
    }

    fn input(path: &str) -> JobInput {
        JobInput { path: PathBuf::from(path), info: video_info(), edits: Edits::default(), audio_track: 0, subtitle: SubChoice::None, forced_only: false, sub_size: 0 }
    }

    fn spec(profile_name: &str, inputs: Vec<JobInput>, out: &str) -> JobSpec {
        JobSpec { inputs, profile: profile(profile_name), output: PathBuf::from(out), hw_encoder: None, hw_decode: None, threads: 0, split_minutes: 0.0 }
    }

    fn pos(args: &[String], what: &str) -> usize {
        args.iter().position(|a| a == what).unwrap_or_else(|| panic!("{what} missing in {args:?}"))
    }

    fn after<'a>(args: &'a [String], what: &str) -> &'a str {
        &args[pos(args, what) + 1]
    }

    #[test]
    fn typed_times_are_understood() {
        assert_eq!(parse_clock("90"), Some(90.0));
        assert_eq!(parse_clock("1:30"), Some(90.0));
        assert_eq!(parse_clock("01:02:03"), Some(3723.0));
        assert_eq!(parse_clock("00:00:01,500"), Some(1.5));
        assert_eq!(parse_clock(" 2:05.5 "), Some(125.5));
        assert_eq!(parse_clock("abc"), None);
        assert_eq!(parse_clock("1:2:3:4"), None);
        assert_eq!(parse_clock("-5"), None);
        assert_eq!(parse_clock(""), None);
    }

    #[test]
    fn clock_shows_tenths_only_when_needed() {
        assert_eq!(fmt_clock(90.0), "01:30");
        assert_eq!(fmt_clock(90.5), "01:30.5");
        assert_eq!(fmt_clock(3723.25), "1:02:03.3");
        assert_eq!(fmt_clock(-4.0), "00:00");
        // What is shown can be typed back.
        assert_eq!(parse_clock(&fmt_clock(125.5)), Some(125.5));
    }

    #[test]
    fn sizes_and_times_read_well() {
        assert_eq!(fmt_time(59.9), "00:59");
        assert_eq!(fmt_time(3600.0), "1:00:00");
        assert_eq!(fmt_bytes(512), "512 B");
        assert_eq!(fmt_bytes(1536), "2 KB");
        assert_eq!(fmt_bytes(5 * 1024 * 1024), "5.0 MB");
        assert_eq!(fmt_bytes(3 * 1024 * 1024 * 1024), "3.00 GB");
    }

    #[test]
    fn only_files_the_job_created_are_cleaned_up() {
        assert!(should_remove_partial(false, 0.0));
        assert!(!should_remove_partial(true, 0.0), "a file that was there before must be kept");
        assert!(!should_remove_partial(false, 10.0), "split parts are left alone");
    }

    #[test]
    fn plain_conversion_command() {
        let s = spec("MP4 – H.264", vec![input("C:/in/clip.mov")], "C:/out/clip.mp4");
        let a = build_args(&s, false, None);
        assert_eq!(after(&a, "-i"), "C:/in/clip.mov");
        assert_eq!(after(&a, "-c:v"), "libx264");
        assert_eq!(after(&a, "-c:a"), "aac");
        assert_eq!(after(&a, "-movflags"), "+faststart");
        assert_eq!(a.last().unwrap(), "C:/out/clip.mp4");
        assert!(!a.iter().any(|x| x == "-ss" || x == "-t"));
        assert!(pos(&a, "-progress") < a.len() - 1);
    }

    #[test]
    fn trim_seeks_before_the_input() {
        let mut inp = input("a.mp4");
        inp.edits.trim_start = 5.0;
        inp.edits.trim_end = 12.5;
        let a = build_args(&spec("MP4 – H.264", vec![inp], "o.mp4"), false, None);
        assert_eq!(after(&a, "-ss"), "5.000");
        assert_eq!(after(&a, "-t"), "7.500");
        assert!(pos(&a, "-ss") < pos(&a, "-i") && pos(&a, "-t") < pos(&a, "-i"));
    }

    #[test]
    fn graphics_card_is_used_only_when_asked() {
        let mut s = spec("MP4 – H.264", vec![input("a.mp4")], "o.mp4");
        s.hw_encoder = Some(HwEncoder { vendor: Vendor::Nvidia, codec: VCodec::H264, name: "h264_nvenc".into() });
        s.hw_decode = Some(Vendor::Nvidia);
        let gpu = build_args(&s, true, None);
        assert_eq!(after(&gpu, "-c:v"), "h264_nvenc");
        assert_eq!(after(&gpu, "-hwaccel"), "cuda");
        let cpu = build_args(&s, false, None);
        assert_eq!(after(&cpu, "-c:v"), "libx264");
        assert!(!cpu.iter().any(|x| x == "-hwaccel"));
    }

    #[test]
    fn hevc_in_mp4_is_tagged_for_apple_players() {
        let a = build_args(&spec("MP4 – H.265 / HEVC", vec![input("a.mkv")], "o.mp4"), false, None);
        assert_eq!(after(&a, "-c:v"), "libx265");
        assert_eq!(after(&a, "-tag:v"), "hvc1");
    }

    #[test]
    fn audio_profile_drops_the_picture() {
        let a = build_args(&spec("MP3 – music (320 kbps)", vec![input("a.mp4")], "o.mp3"), false, None);
        assert!(a.iter().any(|x| x == "-vn"));
        assert_eq!(after(&a, "-c:a"), "libmp3lame");
        assert_eq!(after(&a, "-b:a"), "320k");
        assert!(!a.iter().any(|x| x == "-c:v"));
    }

    #[test]
    fn remux_copies_streams() {
        let a = build_args(&spec("Change container only (instant)", vec![input("a.mp4")], "o.mkv"), true, None);
        assert_eq!(after(&a, "-c"), "copy");
        assert_eq!(after(&a, "-map"), "0");
        assert!(!a.iter().any(|x| x == "-vf" || x == "-hwaccel"));
    }

    #[test]
    fn split_writes_numbered_parts() {
        let mut s = spec("MP4 – H.264", vec![input("a.mp4")], "C:/out/movie.mp4");
        s.split_minutes = 10.0;
        let a = build_args(&s, false, None);
        assert_eq!(after(&a, "-f"), "segment");
        assert_eq!(after(&a, "-segment_time"), "600");
        assert!(a.last().unwrap().ends_with("movie_part%02d.mp4"));
        assert!(!a.iter().any(|x| x == "-movflags"), "faststart does not work with parts");
        assert_eq!(split_pattern(Path::new("x/y.mkv")), PathBuf::from("x/y_part%02d.mkv"));
    }

    #[test]
    fn merge_joins_every_input() {
        let a = build_args(&spec("MP4 – H.264", vec![input("a.mp4"), input("b.mp4")], "o.mp4"), false, None);
        assert_eq!(a.iter().filter(|x| *x == "-i").count(), 2);
        let graph = after(&a, "-filter_complex");
        assert!(graph.contains("concat=n=2:v=1:a=1[vout][aout]"), "{graph}");
        assert!(graph.contains("scale=1920:1080"));
        // Stream-copy merging goes through a list file instead.
        let copy = build_args(&spec("Copy all streams into MKV", vec![input("a.mp4"), input("b.mp4")], "o.mkv"), false, Some(Path::new("list.txt")));
        assert_eq!(after(&copy, "-f"), "concat");
        assert_eq!(after(&copy, "-i"), "list.txt");
    }

    #[test]
    fn edits_become_filters_in_a_fixed_order() {
        let e = Edits { deinterlace: true, crop: [10, 20, 30, 40], rotate: 90, flip_h: true, brightness: 0.1, ..Default::default() };
        let f = e.picture_filters();
        assert_eq!(f[0], "yadif");
        assert_eq!(f[1], "crop=iw-40:ih-60:10:20");
        assert_eq!(f[2], "transpose=1");
        assert_eq!(f[3], "hflip");
        assert!(f[4].starts_with("eq=brightness=0.100"));
        assert!(Edits::default().picture_filters().is_empty());
        assert!(Edits::default().is_default());
    }

    #[test]
    fn trimmed_length() {
        let e = Edits { trim_start: 10.0, trim_end: 25.0, ..Default::default() };
        assert_eq!(e.effective_duration(60.0), 15.0);
        let to_end = Edits { trim_start: 10.0, ..Default::default() };
        assert_eq!(to_end.effective_duration(60.0), 50.0);
        let wrong = Edits { trim_start: 30.0, trim_end: 20.0, ..Default::default() };
        assert_eq!(wrong.effective_duration(60.0), 0.0);
    }

    #[test]
    fn watermark_text_cannot_break_the_filter() {
        let e = Edits { watermark: "50% off: it's \\ here".into(), ..Default::default() };
        let f = e.watermark_filter().unwrap();
        assert!(f.contains("50\\% off\\: it\u{2019}s"), "{f}");
        assert_eq!(f.matches('\'').count() % 2, 0, "quotes must stay balanced: {f}");
        assert!(Edits { watermark: "   ".into(), ..Default::default() }.watermark_filter().is_none());
        // Kurdish text passes through untouched.
        let ku = Edits { watermark: "ڕێگای ئاشتی".into(), ..Default::default() }.watermark_filter().unwrap();
        assert!(ku.contains("ڕێگای ئاشتی"));
    }

    #[test]
    fn paths_inside_filters_are_escaped() {
        assert_eq!(filter_path(Path::new("C:\\Videos\\a,b [x]\\it's.srt")), "C\\:/Videos/a\\,b \\[x\\]/it\\'s.srt");
        let subs = [Track { codec: "subrip".into(), text_based: true, ..Default::default() }];
        let f = text_subtitle_filters(Path::new("D:\\v.mkv"), &SubChoice::Burn(0), &subs, 0.0, 24);
        assert_eq!(f, vec!["subtitles=filename='D\\:/v.mkv':si=0:force_style='Fontsize=24'".to_string()]);
        // After a trim the subtitle clock is shifted back so lines still match.
        let shifted = text_subtitle_filters(Path::new("v.mkv"), &SubChoice::Burn(0), &subs, 4.0, 0);
        assert_eq!(shifted.len(), 3);
        assert_eq!(shifted[0], "setpts=PTS+4.000/TB");
        assert!(text_subtitle_filters(Path::new("v.mkv"), &SubChoice::None, &subs, 0.0, 0).is_empty());
    }

    #[test]
    fn old_subtitle_encodings_are_recognised() {
        let dir = std::env::temp_dir().join(format!("reelforge_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let utf8 = dir.join("utf8.srt");
        std::fs::write(&utf8, "1\n00:00:01,000 --> 00:00:02,000\nسڵاو\n").unwrap();
        assert_eq!(subtitle_charenc(&utf8), None);
        let arabic = dir.join("cp1256.srt");
        std::fs::write(&arabic, [0xD3u8, 0xE1, 0xC7, 0xE3, 0x20, 0xCF, 0xE4, 0xED, 0xC7]).unwrap();
        assert_eq!(subtitle_charenc(&arabic), Some("CP1256"));
        let latin = dir.join("cp1252.srt");
        std::fs::write(&latin, b"Caf\xE9 au lait, s'il vous plait").unwrap();
        assert_eq!(subtitle_charenc(&latin), Some("CP1252"));
        let utf16 = dir.join("utf16.srt");
        std::fs::write(&utf16, [0xFFu8, 0xFE, b'1', 0]).unwrap();
        assert_eq!(subtitle_charenc(&utf16), Some("UTF-16LE"));
        assert_eq!(first_subtitle_time(Path::new("x"), &SubChoice::External(utf8.clone())), Some(1.0));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn output_picture_size() {
        let info = video_info();
        assert_eq!(out_dims(&profile("MP4 – H.264"), &info, &Edits::default()), (1920, 1080));
        assert_eq!(out_dims(&profile("MP4 – tiny file for sharing (720p)"), &info, &Edits::default()), (1280, 720));
        let turned = Edits { rotate: 90, ..Default::default() };
        assert_eq!(out_dims(&profile("MP4 – H.264"), &info, &turned), (1080, 1920));
        let cropped = Edits { crop: [100, 0, 100, 0], ..Default::default() };
        assert_eq!(out_dims(&profile("MP4 – H.264"), &info, &cropped), (1720, 1080));
    }

    #[test]
    fn size_estimate_follows_length_and_quality() {
        let info = video_info();
        let full = estimate_output(&profile("MP4 – H.264"), &info, &Edits::default(), 0).unwrap();
        let half = estimate_output(&profile("MP4 – H.264"), &info, &Edits { trim_end: 30.0, ..Default::default() }, 0).unwrap();
        assert!((half as f64 / full as f64 - 0.5).abs() < 0.01);
        let small = estimate_output(&profile("MP4 – tiny file for sharing (720p)"), &info, &Edits::default(), 0).unwrap();
        assert!(small < full);
        // Never promises more than the source for the same codec and size.
        assert!(full <= info.size * 11 / 10, "{full}");
        let no_length = MediaInfo { duration: 0.0, ..video_info() };
        assert_eq!(estimate_output(&profile("MP4 – H.264"), &no_length, &Edits::default(), 0), None);
    }

    #[test]
    fn track_labels_keep_the_title_as_written() {
        let t = Track { codec: "aac".into(), lang: "kur".into(), title: "دەنگی ڕەسەن".into(), channels: 6, ..Default::default() };
        assert_eq!(t.label(1), "#2 aac [kur] 5.1 – دەنگی ڕەسەن");
    }

    /// Runs a real conversion with FFmpeg; skipped when FFmpeg is not installed.
    #[test]
    fn a_stopped_or_failed_conversion_leaves_no_broken_file() {
        let Some(ff) = ffmpeg_path() else { return };
        let dir = std::env::temp_dir().join(format!("reelforge_job_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let src = dir.join("source.mp4");
        let made = hidden_command(&ff)
            .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc2=s=1920x1080:r=30:d=30", "-c:v", "libx264", "-preset", "ultrafast", "-pix_fmt", "yuv420p"])
            .arg(&src)
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        assert!(made, "could not make the sample clip");
        let ctx = eframe::egui::Context::default();
        let wait = |job: &JobHandle| {
            let t0 = Instant::now();
            while !job.state.lock().unwrap().finished && t0.elapsed().as_secs() < 120 {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            job.state.lock().unwrap().clone()
        };
        let job_for = |source: &Path, out: &Path| {
            let mut inp = input("x");
            inp.path = source.to_path_buf();
            inp.info = probe(source);
            // A slow profile, so there is time to stop it half-way.
            JobSpec { inputs: vec![inp], profile: profile("MKV – best quality archive (H.265 10-bit)"), output: out.to_path_buf(), hw_encoder: None, hw_decode: None, threads: 1, split_minutes: 0.0 }
        };

        // 1. Stopped half-way: the half-written file is removed.
        let out = dir.join("stopped.mkv");
        let job = start_job(job_for(&src, &out), ctx.clone());
        let t0 = Instant::now();
        while !out.exists() && !job.state.lock().unwrap().finished && t0.elapsed().as_secs() < 60 {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert!(out.exists(), "the conversion should have started writing");
        job.cancel();
        let st = wait(&job);
        assert!(st.cancelled && !st.ok);
        assert!(!out.exists(), "the half-written file should be gone");

        // 2. Failed (the source is missing): a file that was already there is kept.
        let keep = dir.join("mine.mkv");
        std::fs::write(&keep, b"my own file").unwrap();
        let st = wait(&start_job(job_for(&dir.join("missing.mp4"), &keep), ctx.clone()));
        assert!(!st.ok && !st.cancelled && !st.error.is_empty());
        assert_eq!(std::fs::read(&keep).unwrap(), b"my own file");

        // 3. Failed with nothing there before: nothing is left behind.
        let none = dir.join("nothing.mkv");
        let st = wait(&start_job(job_for(&dir.join("missing.mp4"), &none), ctx.clone()));
        assert!(!st.ok);
        assert!(!none.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn reading_files_is_limited_and_complete() {
        use std::sync::atomic::AtomicUsize;
        static RUNNING: AtomicUsize = AtomicUsize::new(0);
        static PEAK: AtomicUsize = AtomicUsize::new(0);
        let pool = ProbePool::default();
        for id in 0..40u64 {
            pool.submit_with(
                id,
                PathBuf::from(format!("{id}.mp4")),
                |p| {
                    let now = RUNNING.fetch_add(1, Ordering::SeqCst) + 1;
                    PEAK.fetch_max(now, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    RUNNING.fetch_sub(1, Ordering::SeqCst);
                    MediaInfo { error: p.to_string_lossy().to_string(), ..Default::default() }
                },
                || {},
            );
        }
        let t0 = Instant::now();
        while pool.busy() && t0.elapsed().as_secs() < 20 {
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let got = pool.inbox.lock().unwrap();
        assert_eq!(got.len(), 40);
        let mut ids: Vec<u64> = got.iter().map(|(id, _)| *id).collect();
        ids.sort_unstable();
        assert_eq!(ids, (0..40).collect::<Vec<u64>>());
        assert!(got.iter().all(|(id, info)| info.error == format!("{id}.mp4")));
        let peak = PEAK.load(Ordering::SeqCst);
        assert!((1..=PROBE_WORKERS).contains(&peak), "peak {peak}");
    }
}
