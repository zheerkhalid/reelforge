//! The conversion profile library.
//!
//! The library is generated in code, with modern formats (HEVC, AV1,
//! VP9, ProRes, DNxHR, social-media sizes, 4K/8K, lossless archive, remux).

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Hash)]
pub enum VCodec {
    None,
    Copy,
    H264,
    H265,
    Av1,
    Vp9,
    Vp8,
    Mpeg4,
    Xvid,
    Mpeg2,
    Mpeg1,
    Wmv2,
    H263,
    ProRes,
    DnxHr,
    Mjpeg,
    Ffv1,
    Huffyuv,
    Dv,
    Gif,
    Theora,
    Flv1,
}

impl VCodec {
    pub const ALL: [VCodec; 22] = [
        VCodec::H264,
        VCodec::H265,
        VCodec::Av1,
        VCodec::Vp9,
        VCodec::Vp8,
        VCodec::Mpeg4,
        VCodec::Xvid,
        VCodec::Mpeg2,
        VCodec::Mpeg1,
        VCodec::Wmv2,
        VCodec::H263,
        VCodec::ProRes,
        VCodec::DnxHr,
        VCodec::Mjpeg,
        VCodec::Ffv1,
        VCodec::Huffyuv,
        VCodec::Dv,
        VCodec::Gif,
        VCodec::Theora,
        VCodec::Flv1,
        VCodec::Copy,
        VCodec::None,
    ];

    pub fn label(self) -> &'static str {
        match self {
            VCodec::None => "No video",
            VCodec::Copy => "Keep original (no re-encode)",
            VCodec::H264 => "H.264 / AVC",
            VCodec::H265 => "H.265 / HEVC",
            VCodec::Av1 => "AV1",
            VCodec::Vp9 => "VP9",
            VCodec::Vp8 => "VP8",
            VCodec::Mpeg4 => "MPEG-4 Part 2",
            VCodec::Xvid => "Xvid",
            VCodec::Mpeg2 => "MPEG-2",
            VCodec::Mpeg1 => "MPEG-1",
            VCodec::Wmv2 => "WMV (WMV8)",
            VCodec::H263 => "H.263",
            VCodec::ProRes => "Apple ProRes",
            VCodec::DnxHr => "Avid DNxHR",
            VCodec::Mjpeg => "Motion JPEG",
            VCodec::Ffv1 => "FFV1 (lossless)",
            VCodec::Huffyuv => "HuffYUV (lossless)",
            VCodec::Dv => "DV",
            VCodec::Gif => "Animated GIF",
            VCodec::Theora => "Theora",
            VCodec::Flv1 => "Sorenson Spark (FLV1)",
        }
    }

    /// Codecs a graphics card can encode (NVENC / AMF / Quick Sync).
    pub fn gpu_capable(self) -> bool {
        matches!(self, VCodec::H264 | VCodec::H265 | VCodec::Av1)
    }

    pub fn cpu_encoder(self) -> &'static str {
        match self {
            VCodec::None | VCodec::Copy => "copy",
            VCodec::H264 => "libx264",
            VCodec::H265 => "libx265",
            VCodec::Av1 => "libsvtav1",
            VCodec::Vp9 => "libvpx-vp9",
            VCodec::Vp8 => "libvpx",
            VCodec::Mpeg4 => "mpeg4",
            VCodec::Xvid => "libxvid",
            VCodec::Mpeg2 => "mpeg2video",
            VCodec::Mpeg1 => "mpeg1video",
            VCodec::Wmv2 => "wmv2",
            VCodec::H263 => "h263",
            VCodec::ProRes => "prores_ks",
            VCodec::DnxHr => "dnxhd",
            VCodec::Mjpeg => "mjpeg",
            VCodec::Ffv1 => "ffv1",
            VCodec::Huffyuv => "huffyuv",
            VCodec::Dv => "dvvideo",
            VCodec::Gif => "gif",
            VCodec::Theora => "libtheora",
            VCodec::Flv1 => "flv",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Hash)]
pub enum ACodec {
    None,
    Copy,
    Aac,
    Mp3,
    Ac3,
    Eac3,
    Opus,
    Vorbis,
    Flac,
    Alac,
    Pcm16,
    Pcm24,
    Pcm16Be,
    Wma,
    AmrNb,
    Mp2,
}

impl ACodec {
    pub const ALL: [ACodec; 16] = [
        ACodec::Aac,
        ACodec::Mp3,
        ACodec::Ac3,
        ACodec::Eac3,
        ACodec::Opus,
        ACodec::Vorbis,
        ACodec::Flac,
        ACodec::Alac,
        ACodec::Pcm16,
        ACodec::Pcm24,
        ACodec::Pcm16Be,
        ACodec::Wma,
        ACodec::AmrNb,
        ACodec::Mp2,
        ACodec::Copy,
        ACodec::None,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ACodec::None => "No audio",
            ACodec::Copy => "Keep original (no re-encode)",
            ACodec::Aac => "AAC",
            ACodec::Mp3 => "MP3",
            ACodec::Ac3 => "Dolby Digital (AC-3)",
            ACodec::Eac3 => "Dolby Digital Plus (E-AC-3)",
            ACodec::Opus => "Opus",
            ACodec::Vorbis => "Vorbis",
            ACodec::Flac => "FLAC (lossless)",
            ACodec::Alac => "Apple Lossless (ALAC)",
            ACodec::Pcm16 => "PCM 16-bit",
            ACodec::Pcm24 => "PCM 24-bit",
            ACodec::Pcm16Be => "PCM 16-bit (big-endian)",
            ACodec::Wma => "WMA",
            ACodec::AmrNb => "AMR-NB",
            ACodec::Mp2 => "MPEG Audio Layer 2",
        }
    }

    pub fn encoder(self) -> &'static str {
        match self {
            ACodec::None | ACodec::Copy => "copy",
            ACodec::Aac => "aac",
            ACodec::Mp3 => "libmp3lame",
            ACodec::Ac3 => "ac3",
            ACodec::Eac3 => "eac3",
            ACodec::Opus => "libopus",
            ACodec::Vorbis => "libvorbis",
            ACodec::Flac => "flac",
            ACodec::Alac => "alac",
            ACodec::Pcm16 => "pcm_s16le",
            ACodec::Pcm24 => "pcm_s24le",
            ACodec::Pcm16Be => "pcm_s16be",
            ACodec::Wma => "wmav2",
            ACodec::AmrNb => "libopencore_amrnb",
            ACodec::Mp2 => "mp2",
        }
    }

    pub fn lossless(self) -> bool {
        matches!(
            self,
            ACodec::Flac | ACodec::Alac | ACodec::Pcm16 | ACodec::Pcm24 | ACodec::Pcm16Be
        )
    }
}

/// Video quality: high, medium and low, plus "Best" and a fixed bitrate.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Hash)]
pub enum Quality {
    Best,
    High,
    Medium,
    Low,
    Bitrate,
}

impl Quality {
    pub const ALL: [Quality; 5] = [
        Quality::Best,
        Quality::High,
        Quality::Medium,
        Quality::Low,
        Quality::Bitrate,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Quality::Best => "Best (bigger file)",
            Quality::High => "High",
            Quality::Medium => "Medium (balanced)",
            Quality::Low => "Low (small file)",
            Quality::Bitrate => "Fixed bitrate",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Hash)]
pub enum Fit {
    /// Scale to the height, keep the aspect ratio (width follows).
    Height,
    /// Fit inside WxH and add black bars.
    Pad,
    /// Fill WxH and cut the overflow.
    Fill,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Hash)]
pub enum Stereo {
    None,
    AnaglyphFromSbs,
    AnaglyphFromTb,
    SbsToTb,
    TbToSbs,
    SbsHalf,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Profile {
    pub id: u32,
    pub category: String,
    pub name: String,
    pub ext: String,
    pub vcodec: VCodec,
    pub acodec: ACodec,
    /// 0 = keep the original size.
    pub width: u32,
    pub height: u32,
    pub fit: Fit,
    /// 0 = keep the original frame rate.
    pub fps: f32,
    pub quality: Quality,
    /// Used when quality is Bitrate.
    pub vkbps: u32,
    /// 0 = encoder default / lossless.
    pub akbps: u32,
    /// 0 = keep.
    pub sample_rate: u32,
    /// 0 = keep.
    pub channels: u8,
    pub ten_bit: bool,
    pub stereo: Stereo,
    pub note: String,
    /// Extra encoder options (e.g. ProRes profile, DVD target).
    pub extra: Vec<String>,
    pub custom: bool,
}

#[allow(dead_code)]
impl Profile {
    pub fn is_audio_only(&self) -> bool {
        self.vcodec == VCodec::None
    }
    pub fn is_remux(&self) -> bool {
        self.vcodec == VCodec::Copy && matches!(self.acodec, ACodec::Copy | ACodec::None)
    }
    pub fn summary(&self) -> String {
        let mut s = String::new();
        if self.vcodec != VCodec::None {
            s.push_str(self.vcodec.label());
            if self.vcodec != VCodec::Copy {
                if self.height > 0 {
                    if self.width > 0 && self.fit != Fit::Height {
                        s.push_str(&format!(" · {}×{}", self.width, self.height));
                    } else {
                        s.push_str(&format!(" · {}p", self.height));
                    }
                }
                if self.fps > 0.0 {
                    s.push_str(&format!(" · {} fps", trim_float(self.fps)));
                }
                if self.quality == Quality::Bitrate {
                    s.push_str(&format!(" · {} kbps", self.vkbps));
                }
                if self.ten_bit {
                    s.push_str(" · 10-bit");
                }
            }
        }
        if self.acodec != ACodec::None {
            if !s.is_empty() {
                s.push_str("  |  ");
            }
            s.push_str(self.acodec.label());
            if self.akbps > 0 && !self.acodec.lossless() && self.acodec != ACodec::Copy {
                s.push_str(&format!(" {} kbps", self.akbps));
            }
        }
        s
    }
}

pub fn trim_float(v: f32) -> String {
    let s = format!("{:.3}", v);
    s.trim_end_matches('0').trim_end_matches('.').to_string()
}

pub const CATEGORIES: &[(&str, &str)] = &[
    ("Recommended", "⭐"),
    ("Common Video", "🎬"),
    ("Common Audio", "🎵"),
    ("HD & 4K / 8K", "📺"),
    ("Social & Web", "🌐"),
    ("Apple", "📱"),
    ("Android", "📱"),
    ("TV & Media Players", "📺"),
    ("Game Consoles", "🎮"),
    ("Editing Software", "✂"),
    ("Lossless & Archive", "🗄"),
    ("Remux (no quality loss)", "⚡"),
    ("3D Video", "👓"),
    ("DVD, VCD & Blu-ray", "💿"),
    ("DV & Interlaced", "📼"),
    ("Windows & Office", "🖥"),
    ("Older Phones & Players", "📟"),
    ("Flash & Legacy Web", "🕸"),
    ("Audio for Devices", "🎧"),
    ("Cameras & Drones", "📷"),
    ("My Profiles", "★"),
];

struct B {
    list: Vec<Profile>,
    next: u32,
}

#[allow(clippy::too_many_arguments)]
impl B {
    fn add(
        &mut self,
        cat: &str,
        name: &str,
        ext: &str,
        v: VCodec,
        a: ACodec,
        w: u32,
        h: u32,
        fps: f32,
        q: Quality,
        akbps: u32,
        note: &str,
    ) -> &mut Profile {
        self.next += 1;
        self.list.push(Profile {
            id: stable_id(cat, name),
            category: cat.to_string(),
            name: name.to_string(),
            ext: ext.to_string(),
            vcodec: v,
            acodec: a,
            width: w,
            height: h,
            fit: if w == 0 { Fit::Height } else { Fit::Pad },
            fps,
            quality: q,
            vkbps: 0,
            akbps,
            sample_rate: 0,
            channels: if a == ACodec::None || a == ACodec::Copy { 0 } else { 2 },
            ten_bit: false,
            stereo: Stereo::None,
            note: note.to_string(),
            extra: vec![],
            custom: false,
        });
        self.list.last_mut().unwrap()
    }
    fn audio(&mut self, cat: &str, name: &str, ext: &str, a: ACodec, kbps: u32, rate: u32, ch: u8, note: &str) {
        let p = self.add(cat, name, ext, VCodec::None, a, 0, 0, 0.0, Quality::High, kbps, note);
        p.sample_rate = rate;
        p.channels = ch;
    }
}

fn res_name(h: u32) -> &'static str {
    match h {
        360 => "360p",
        480 => "480p SD",
        540 => "540p",
        576 => "576p",
        720 => "720p HD",
        1080 => "1080p Full HD",
        1440 => "1440p QHD",
        2160 => "4K UHD",
        4320 => "8K UHD",
        _ => "",
    }
}

pub fn library() -> Vec<Profile> {
    use ACodec as A;
    use Quality::*;
    use VCodec as V;
    let mut b = B { list: Vec::new(), next: 1000 };

    // ---------------- Recommended (one-click picks) ----------------
    let r = "Recommended";
    b.add(r, "MP4 – works everywhere (H.264)", "mp4", V::H264, A::Aac, 0, 0, 0.0, High, 160, "Plays on phones, TVs, browsers and every computer. Keeps the original size.");
    b.add(r, "MP4 – HEVC / H.265: smaller file, same quality", "mp4", V::H265, A::Aac, 0, 0, 0.0, High, 160, "About half the size of H.264 at the same quality. Plays on recent devices.");
    b.add(r, "MP4 – HEVC / H.265: very small file", "mp4", V::H265, A::Aac, 0, 0, 0.0, Medium, 128, "The smallest good-looking file. Keeps the original size; ideal for storing or sending long videos.");
    b.add(r, "MP4 – tiny file for sharing (720p)", "mp4", V::H264, A::Aac, 0, 720, 0.0, Low, 96, "Good for messaging apps and email.");
    b.add(r, "MKV – best quality archive (H.265 10-bit)", "mkv", V::H265, A::Flac, 0, 0, 0.0, Best, 0, "Near-lossless picture, lossless sound.").ten_bit = true;
    b.add(r, "AV1 – newest, smallest (MP4)", "mp4", V::Av1, A::Opus, 0, 0, 0.0, High, 128, "Best compression available. Needs a recent player.");
    b.add(r, "Change container only (instant)", "mkv", V::Copy, A::Copy, 0, 0, 0.0, High, 0, "Copies the streams without re-encoding: no quality loss, takes seconds.");
    b.audio(r, "MP3 – music (320 kbps)", "mp3", A::Mp3, 320, 44100, 2, "Extract or convert the sound to high-quality MP3.");
    b.audio(r, "FLAC – lossless audio", "flac", A::Flac, 0, 0, 0, "Perfect copy of the sound, smaller than WAV.");
    b.add(r, "YouTube 1080p", "mp4", V::H264, A::Aac, 0, 1080, 0.0, High, 192, "Matches YouTube's recommended upload settings.");
    b.add(r, "Phone vertical video (1080×1920)", "mp4", V::H264, A::Aac, 1080, 1920, 30.0, High, 128, "For Reels, Shorts and stories.");
    b.add(r, "Animated GIF (480 px)", "gif", V::Gif, A::None, 480, 0, 12.0, Medium, 0, "Short clips as a looping GIF.").fit = Fit::Height;

    // ---------------- Common video ----------------
    let c = "Common Video";
    b.add(c, "MP4 – H.264", "mp4", V::H264, A::Aac, 0, 0, 0.0, High, 160, "The most compatible video format.");
    b.add(c, "MP4 – H.265 / HEVC", "mp4", V::H265, A::Aac, 0, 0, 0.0, High, 160, "Half the size of H.264 at the same quality.");
    b.add(c, "MP4 – AV1", "mp4", V::Av1, A::Opus, 0, 0, 0.0, High, 128, "Royalty-free, best compression.");
    b.add(c, "MP4 – H.264 Baseline (old devices)", "mp4", V::H264, A::Aac, 0, 0, 0.0, High, 128, "Simplest H.264 for very old phones.").extra = s(&["-profile:v", "baseline", "-level", "3.1"]);
    b.add(c, "MP4 – H.264 Main profile", "mp4", V::H264, A::Aac, 0, 0, 0.0, High, 160, "").extra = s(&["-profile:v", "main"]);
    b.add(c, "MP4 – H.264 High profile", "mp4", V::H264, A::Aac, 0, 0, 0.0, High, 160, "").extra = s(&["-profile:v", "high"]);
    b.add(c, "MP4 – MPEG-4 (Xvid style)", "mp4", V::Mpeg4, A::Aac, 0, 0, 0.0, High, 128, "Common MPEG-4 video for older Android and Apple devices.");
    b.add(c, "MKV – H.264", "mkv", V::H264, A::Aac, 0, 0, 0.0, High, 192, "Matroska keeps several audio and subtitle tracks.");
    b.add(c, "MKV – H.265 / HEVC", "mkv", V::H265, A::Aac, 0, 0, 0.0, High, 192, "");
    b.add(c, "MKV – AV1", "mkv", V::Av1, A::Opus, 0, 0, 0.0, High, 160, "");
    b.add(c, "MKV – VP9", "mkv", V::Vp9, A::Opus, 0, 0, 0.0, High, 160, "");
    b.add(c, "MOV – QuickTime H.264", "mov", V::H264, A::Aac, 0, 0, 0.0, High, 192, "Apple QuickTime movie.");
    b.add(c, "MOV – QuickTime H.265", "mov", V::H265, A::Aac, 0, 0, 0.0, High, 192, "").extra = s(&["-tag:v", "hvc1"]);
    b.add(c, "M4V – MPEG-4 Video", "m4v", V::H264, A::Aac, 0, 0, 0.0, High, 160, "iTunes-style MPEG-4 video.");
    b.add(c, "AVI – Xvid", "avi", V::Xvid, A::Mp3, 0, 0, 0.0, High, 192, "Classic AVI for DVD players with USB.");
    b.add(c, "AVI – DivX compatible (MPEG-4)", "avi", V::Mpeg4, A::Mp3, 0, 0, 0.0, High, 192, "").extra = s(&["-vtag", "DIVX"]);
    b.add(c, "AVI – H.264", "avi", V::H264, A::Mp3, 0, 0, 0.0, High, 192, "");
    b.add(c, "WMV – Windows Media (WMV8)", "wmv", V::Wmv2, A::Wma, 0, 0, 0.0, High, 192, "Fast conversion with the wmv2 codec.");
    b.add(c, "ASF – Windows Media", "asf", V::Wmv2, A::Wma, 0, 0, 0.0, High, 160, "");
    b.add(c, "WebM – VP9", "webm", V::Vp9, A::Opus, 0, 0, 0.0, High, 128, "Open format for HTML5 video.");
    b.add(c, "WebM – VP8", "webm", V::Vp8, A::Vorbis, 0, 0, 0.0, High, 128, "");
    b.add(c, "WebM – AV1", "webm", V::Av1, A::Opus, 0, 0, 0.0, High, 128, "");
    b.add(c, "MPEG-2 Video (MPG)", "mpg", V::Mpeg2, A::Mp2, 0, 0, 0.0, High, 224, "Optimised for television.");
    b.add(c, "MPEG-1 Video (MPG)", "mpg", V::Mpeg1, A::Mp2, 0, 0, 0.0, High, 192, "");
    b.add(c, "TS – Transport stream H.264", "ts", V::H264, A::Aac, 0, 0, 0.0, High, 192, "Broadcast / IPTV style stream.");
    b.add(c, "M2TS – H.264", "m2ts", V::H264, A::Ac3, 0, 0, 0.0, High, 384, "");
    b.add(c, "OGV – Theora", "ogv", V::Theora, A::Vorbis, 0, 0, 0.0, High, 128, "");
    b.add(c, "3GP – H.263 / AMR", "3gp", V::H263, A::AmrNb, 176, 144, 15.0, Medium, 12, "For very old mobile phones.").sample_rate = 8000;
    b.add(c, "3GP – MPEG-4 / AAC", "3gp", V::Mpeg4, A::Aac, 352, 288, 15.0, Medium, 64, "");
    b.add(c, "3G2 – MPEG-4 / AAC", "3g2", V::Mpeg4, A::Aac, 352, 288, 15.0, Medium, 64, "");
    b.add(c, "FLV – H.264", "flv", V::H264, A::Aac, 0, 0, 0.0, High, 128, "");

    // ---------------- Common audio ----------------
    let a = "Common Audio";
    for k in [320, 256, 192, 128, 96] {
        b.audio(a, &format!("MP3 – {} kbps", k), "mp3", A::Mp3, k, 44100, 2, "The most compatible audio format.");
    }
    for k in [320, 256, 192, 128] {
        b.audio(a, &format!("AAC (M4A) – {} kbps", k), "m4a", A::Aac, k, 48000, 2, "Better than MP3 at the same size.");
    }
    b.audio(a, "FLAC – lossless", "flac", A::Flac, 0, 0, 0, "");
    b.audio(a, "FLAC – lossless 24-bit / 96 kHz", "flac", A::Flac, 0, 96000, 0, "");
    b.audio(a, "ALAC – Apple Lossless (M4A)", "m4a", A::Alac, 0, 0, 0, "");
    b.audio(a, "WAV – 16-bit PCM", "wav", A::Pcm16, 0, 44100, 2, "Uncompressed CD quality.");
    b.audio(a, "WAV – 24-bit PCM", "wav", A::Pcm24, 0, 48000, 2, "Uncompressed studio quality.");
    b.audio(a, "AIFF – 16-bit PCM", "aiff", A::Pcm16Be, 0, 44100, 2, "");
    for k in [256, 160, 96, 64] {
        b.audio(a, &format!("Opus – {} kbps", k), "opus", A::Opus, k, 48000, 2, "Excellent quality at low bitrates.");
    }
    b.audio(a, "OGG Vorbis – 192 kbps", "ogg", A::Vorbis, 192, 44100, 2, "");
    b.audio(a, "OGG Vorbis – 128 kbps", "ogg", A::Vorbis, 128, 44100, 2, "");
    b.audio(a, "AC-3 Dolby Digital 5.1", "ac3", A::Ac3, 448, 48000, 6, "");
    b.audio(a, "AC-3 Dolby Digital stereo", "ac3", A::Ac3, 192, 48000, 2, "");
    b.audio(a, "E-AC-3 Dolby Digital Plus 5.1", "eac3", A::Eac3, 640, 48000, 6, "");
    b.audio(a, "WMA – Windows Media Audio", "wma", A::Wma, 192, 44100, 2, "");
    b.audio(a, "MP2 – MPEG Layer 2", "mp2", A::Mp2, 224, 48000, 2, "");
    b.audio(a, "AMR – voice (8 kHz mono)", "amr", A::AmrNb, 12, 8000, 1, "");

    // ---------------- HD & 4K/8K ----------------
    let h = "HD & 4K / 8K";
    for (codec, cname, acodec) in [(V::H264, "H.264", A::Aac), (V::H265, "H.265", A::Aac), (V::Av1, "AV1", A::Opus)] {
        for height in [480u32, 720, 1080, 1440, 2160, 4320] {
            if codec == V::H264 && height == 4320 {
                continue;
            }
            for (ext, ename) in [("mp4", "MP4"), ("mkv", "MKV")] {
                let p = b.add(h, &format!("{} {} – {}", res_name(height), cname, ename), ext, codec, acodec, 0, height, 0.0, High, if height >= 1080 { 192 } else { 160 }, "");
                if codec == V::H265 && ext == "mp4" {
                    p.extra = s(&["-tag:v", "hvc1"]);
                }
            }
        }
    }
    for (codec, cname) in [(V::H264, "H.264"), (V::H265, "H.265")] {
        for height in [1080u32, 1440, 2160] {
            b.add(h, &format!("{} 60 fps {} – MP4", res_name(height), cname), "mp4", codec, A::Aac, 0, height, 60.0, High, 192, "Smooth motion for games and sports.");
        }
    }
    for height in [720u32, 1080, 1440, 2160] {
        b.add(h, &format!("{} VP9 – WebM", res_name(height)), "webm", V::Vp9, A::Opus, 0, height, 0.0, High, 160, "");
    }
    for height in [1080u32, 2160] {
        b.add(h, &format!("{} HDR-ready H.265 10-bit – MKV", res_name(height)), "mkv", V::H265, A::Eac3, 0, height, 0.0, High, 640, "10-bit colour keeps HDR gradients smooth.").ten_bit = true;
        b.add(h, &format!("{} AV1 10-bit – MKV", res_name(height)), "mkv", V::Av1, A::Opus, 0, height, 0.0, High, 256, "").ten_bit = true;
    }
    b.add(h, "1080p H.264 + Dolby 5.1 – MKV", "mkv", V::H264, A::Ac3, 0, 1080, 0.0, High, 448, "").channels = 6;
    b.add(h, "4K H.265 + Dolby 5.1 – MKV", "mkv", V::H265, A::Eac3, 0, 2160, 0.0, High, 640, "").channels = 6;

    // ---------------- Social & Web ----------------
    let w = "Social & Web";
    for (name, height, fps, k) in [("YouTube 720p", 720u32, 0.0, 160u32), ("YouTube 1080p", 1080, 0.0, 192), ("YouTube 1440p", 1440, 0.0, 192), ("YouTube 4K", 2160, 0.0, 256), ("YouTube 1080p 60 fps", 1080, 60.0, 192)] {
        b.add(w, name, "mp4", V::H264, A::Aac, 0, height, fps, High, k, "Recommended upload settings.");
    }
    b.add(w, "YouTube 4K HEVC", "mp4", V::H265, A::Aac, 0, 2160, 0.0, High, 256, "").extra = s(&["-tag:v", "hvc1"]);
    b.add(w, "YouTube Shorts (1080×1920)", "mp4", V::H264, A::Aac, 1080, 1920, 30.0, High, 160, "");
    b.add(w, "TikTok (1080×1920)", "mp4", V::H264, A::Aac, 1080, 1920, 30.0, High, 128, "");
    b.add(w, "Instagram Reels (1080×1920)", "mp4", V::H264, A::Aac, 1080, 1920, 30.0, High, 128, "");
    b.add(w, "Instagram Feed square (1080×1080)", "mp4", V::H264, A::Aac, 1080, 1080, 30.0, High, 128, "");
    b.add(w, "Instagram Feed portrait (1080×1350)", "mp4", V::H264, A::Aac, 1080, 1350, 30.0, High, 128, "");
    b.add(w, "Facebook 1080p", "mp4", V::H264, A::Aac, 0, 1080, 30.0, High, 128, "");
    b.add(w, "Facebook Stories (1080×1920)", "mp4", V::H264, A::Aac, 1080, 1920, 30.0, High, 128, "");
    b.add(w, "X / Twitter (1280×720)", "mp4", V::H264, A::Aac, 1280, 720, 30.0, High, 128, "");
    b.add(w, "X / Twitter vertical (720×1280)", "mp4", V::H264, A::Aac, 720, 1280, 30.0, High, 128, "");
    b.add(w, "Vimeo 1080p", "mp4", V::H264, A::Aac, 0, 1080, 0.0, Best, 320, "");
    b.add(w, "Vimeo 4K", "mp4", V::H265, A::Aac, 0, 2160, 0.0, Best, 320, "").extra = s(&["-tag:v", "hvc1"]);
    b.add(w, "LinkedIn (1080p)", "mp4", V::H264, A::Aac, 0, 1080, 30.0, High, 128, "");
    b.add(w, "Twitch clip (1080p 60)", "mp4", V::H264, A::Aac, 0, 1080, 60.0, High, 160, "");
    b.add(w, "WhatsApp (small 480p)", "mp4", V::H264, A::Aac, 0, 480, 30.0, Low, 96, "").extra = s(&["-profile:v", "baseline"]);
    b.add(w, "Telegram (720p)", "mp4", V::H264, A::Aac, 0, 720, 30.0, Medium, 128, "");
    b.add(w, "Discord small (720p, low)", "mp4", V::H264, A::Aac, 0, 720, 30.0, Low, 96, "Keeps files small for upload limits.");
    b.add(w, "Discord small (480p, very low)", "mp4", V::H264, A::Aac, 0, 480, 30.0, Low, 64, "");
    b.add(w, "Email attachment (360p)", "mp4", V::H264, A::Aac, 0, 360, 24.0, Low, 64, "");
    b.add(w, "Website HTML5 – MP4 (fast start)", "mp4", V::H264, A::Aac, 0, 1080, 0.0, Medium, 128, "Starts playing before the download ends.");
    b.add(w, "Website HTML5 – WebM VP9", "webm", V::Vp9, A::Opus, 0, 1080, 0.0, Medium, 128, "");
    b.add(w, "Website background loop (no sound)", "mp4", V::H264, A::None, 0, 720, 30.0, Low, 0, "");
    for (wd, fps, q, name) in [(320u32, 10.0, Low, "small 320 px"), (480, 12.0, Medium, "medium 480 px"), (640, 15.0, High, "large 640 px")] {
        let p = b.add(w, &format!("Animated GIF – {}", name), "gif", V::Gif, A::None, wd, 0, fps, q, 0, "");
        p.fit = Fit::Height;
    }
    let p = b.add(w, "Animated WebP (480 px)", "webp", V::Gif, A::None, 480, 0, 15.0, Medium, 0, "");
    p.fit = Fit::Height;
    p.extra = s(&["-c:v", "libwebp_anim", "-loop", "0"]);

    // ---------------- Apple ----------------
    let ap = "Apple";
    for (name, wd, ht, codec) in [
        ("iPhone – 1080p H.264", 0u32, 1080u32, V::H264),
        ("iPhone – 4K HEVC", 0, 2160, V::H265),
        ("iPhone – 1080p HEVC (small)", 0, 1080, V::H265),
        ("iPhone – 720p H.264", 0, 720, V::H264),
        ("iPhone – older models (640×480)", 640, 480, V::H264),
        ("iPad – 1080p H.264", 0, 1080, V::H264),
        ("iPad Pro – 4K HEVC", 0, 2160, V::H265),
        ("iPad – 720p H.264", 0, 720, V::H264),
        ("iPad mini (1024×768)", 1024, 768, V::H264),
        ("Apple TV – 1080p", 0, 1080, V::H264),
        ("Apple TV 4K – HEVC", 0, 2160, V::H265),
        ("Apple TV – 720p", 0, 720, V::H264),
        ("iPod – video (640×480)", 640, 480, V::H264),
        ("iPod nano / small (320×240)", 320, 240, V::H264),
        ("Mac – QuickTime 1080p", 0, 1080, V::H264),
        ("Mac – QuickTime 4K HEVC", 0, 2160, V::H265),
        ("Apple Vision – 4K HEVC", 0, 2160, V::H265),
    ] {
        let ext = if name.starts_with("Mac") { "mov" } else { "m4v" };
        let p = b.add(ap, name, ext, codec, A::Aac, wd, ht, 0.0, High, 160, "");
        if codec == V::H265 {
            p.extra = s(&["-tag:v", "hvc1"]);
        } else if wd == 320 || wd == 640 {
            p.extra = s(&["-profile:v", "baseline", "-level", "3.0"]);
        }
    }
    b.add(ap, "Apple TV – 1080p + Dolby 5.1", "m4v", V::H264, A::Ac3, 0, 1080, 0.0, High, 448, "").channels = 6;
    b.audio(ap, "iPhone ringtone (M4R)", "m4r", A::Aac, 192, 44100, 2, "Keep it under 40 seconds (use Edit → Trim).");
    b.audio(ap, "Apple Music / iTunes (AAC 256)", "m4a", A::Aac, 256, 44100, 2, "");

    // ---------------- Android ----------------
    let an = "Android";
    for (name, wd, ht, codec, k) in [
        ("Android phone – 1080p H.264", 0u32, 1080u32, V::H264, 160u32),
        ("Android phone – 1440p H.264", 0, 1440, V::H264, 192),
        ("Android phone – 720p H.264", 0, 720, V::H264, 128),
        ("Android phone – 480p H.264", 0, 480, V::H264, 128),
        ("Android phone – 1080p HEVC", 0, 1080, V::H265, 160),
        ("Android phone – 4K HEVC", 0, 2160, V::H265, 192),
        ("Android phone – 1080p AV1", 0, 1080, V::Av1, 128),
        ("Android tablet – 1080p", 0, 1080, V::H264, 160),
        ("Android tablet – 1200p (1920×1200)", 1920, 1200, V::H264, 160),
        ("Android tablet – 1600p (2560×1600)", 2560, 1600, V::H265, 192),
        ("Android tablet – 800p (1280×800)", 1280, 800, V::H264, 128),
        ("Samsung Galaxy – 1080p", 0, 1080, V::H264, 160),
        ("Samsung Galaxy – 4K HEVC", 0, 2160, V::H265, 192),
        ("Google Pixel – 1080p HEVC", 0, 1080, V::H265, 160),
        ("Small-screen phone (854×480)", 854, 480, V::H264, 96),
        ("Budget phone (640×360)", 640, 360, V::H264, 96),
        ("Android – WebM VP9 1080p", 0, 1080, V::Vp9, 128),
        ("Android – MKV 1080p", 0, 1080, V::H264, 160),
        ("Android – old devices (MPEG-4 480p)", 0, 480, V::Mpeg4, 128),
    ] {
        let ext = if name.contains("WebM") { "webm" } else if name.contains("MKV") { "mkv" } else { "mp4" };
        let acodec = if codec == V::Vp9 || codec == V::Av1 { A::Opus } else { A::Aac };
        b.add(an, name, ext, codec, acodec, wd, ht, 0.0, High, k, "");
    }

    // ---------------- TV & Media players ----------------
    let tv = "TV & Media Players";
    for (name, ext, codec, ht, ac, k) in [
        ("Smart TV – 1080p H.264", "mp4", V::H264, 1080u32, A::Aac, 192u32),
        ("Smart TV – 4K HEVC", "mp4", V::H265, 2160, A::Eac3, 640),
        ("Smart TV – 4K HEVC 10-bit (HDR)", "mkv", V::H265, 2160, A::Eac3, 640),
        ("Smart TV – USB stick (AVI Xvid)", "avi", V::Xvid, 0, A::Mp3, 192),
        ("Smart TV – 720p H.264", "mp4", V::H264, 720, A::Aac, 160),
        ("Streaming stick – 1080p", "mp4", V::H264, 1080, A::Aac, 192),
        ("Streaming stick – 4K HEVC", "mp4", V::H265, 2160, A::Eac3, 640),
        ("Chromecast – 1080p", "mp4", V::H264, 1080, A::Aac, 192),
        ("Chromecast – 4K VP9", "webm", V::Vp9, 2160, A::Opus, 192),
        ("Roku – 1080p", "mkv", V::H264, 1080, A::Aac, 192),
        ("Fire TV – 4K HEVC", "mp4", V::H265, 2160, A::Eac3, 640),
        ("Plex / Jellyfin / Kodi – 1080p MKV", "mkv", V::H264, 1080, A::Ac3, 448),
        ("Plex / Jellyfin / Kodi – 4K HEVC MKV", "mkv", V::H265, 2160, A::Eac3, 640),
        ("HD media player – TS 1080p", "ts", V::H264, 1080, A::Ac3, 448),
        ("HD media player – M2TS 1080p", "m2ts", V::H264, 1080, A::Ac3, 448),
        ("DLNA media server – MPEG-2 TS", "ts", V::Mpeg2, 0, A::Ac3, 384),
        ("Projector – 720p", "mp4", V::H264, 720, A::Aac, 160),
        ("Car screen (800×480)", "mp4", V::H264, 800, A::Aac, 128),
    ] {
        let p = b.add(tv, name, ext, codec, ac, if ht == 480 { 800 } else { 0 }, if name.starts_with("Car") { 480 } else { ht }, 0.0, High, k, "");
        if name.contains("10-bit") {
            p.ten_bit = true;
        }
        if codec == V::H265 && ext == "mp4" {
            p.extra = s(&["-tag:v", "hvc1"]);
        }
        if name.starts_with("Car") {
            p.width = 800;
        }
    }

    // ---------------- Game consoles ----------------
    let g = "Game Consoles";
    b.add(g, "PlayStation 5 – 4K HEVC", "mp4", V::H265, A::Aac, 0, 2160, 0.0, High, 192, "");
    b.add(g, "PlayStation 5 – 1080p", "mp4", V::H264, A::Aac, 0, 1080, 0.0, High, 192, "");
    b.add(g, "PlayStation 4 – 1080p", "mp4", V::H264, A::Aac, 0, 1080, 0.0, High, 192, "");
    b.add(g, "PlayStation 3 – 1080p", "mp4", V::H264, A::Aac, 0, 1080, 0.0, High, 160, "").extra = s(&["-level", "4.1"]);
    b.add(g, "PlayStation 3 – AVI", "avi", V::Xvid, A::Mp3, 0, 720, 0.0, High, 192, "");
    b.add(g, "PSP (480×272)", "mp4", V::H264, A::Aac, 480, 272, 29.97, Medium, 128, "").extra = s(&["-profile:v", "main", "-level", "3.0"]);
    b.add(g, "PS Vita (960×544)", "mp4", V::H264, A::Aac, 960, 544, 0.0, High, 128, "");
    b.add(g, "Xbox Series X|S – 4K HEVC", "mp4", V::H265, A::Aac, 0, 2160, 0.0, High, 192, "");
    b.add(g, "Xbox Series / One – 1080p", "mp4", V::H264, A::Aac, 0, 1080, 0.0, High, 192, "");
    b.add(g, "Xbox 360 – WMV", "wmv", V::Wmv2, A::Wma, 0, 720, 0.0, High, 192, "");
    b.add(g, "Nintendo Switch – 720p", "mp4", V::H264, A::Aac, 0, 720, 30.0, High, 128, "");
    b.add(g, "Nintendo Wii – MJPEG AVI", "avi", V::Mjpeg, A::Pcm16, 640, 480, 30.0, Medium, 0, "").sample_rate = 32000;
    b.add(g, "Steam Deck – 800p", "mp4", V::H264, A::Aac, 1280, 800, 0.0, High, 160, "");

    // ---------------- Editing software ----------------
    let e = "Editing Software";
    for (name, prof) in [("ProRes 422 Proxy", "0"), ("ProRes 422 LT", "1"), ("ProRes 422", "2"), ("ProRes 422 HQ", "3"), ("ProRes 4444", "4"), ("ProRes 4444 XQ", "5")] {
        let p = b.add(e, &format!("Final Cut Pro – Apple {} (MOV)", name), "mov", V::ProRes, A::Pcm16, 0, 0, 0.0, Best, 0, "Edit-friendly: every frame is a keyframe.");
        let pix = if prof == "4" || prof == "5" { "yuva444p10le" } else { "yuv422p10le" };
        p.extra = s(&["-profile:v", prof, "-vendor", "apl0", "-pix_fmt", pix]);
    }
    for (name, prof, pix) in [("DNxHR LB", "dnxhr_lb", "yuv422p"), ("DNxHR SQ", "dnxhr_sq", "yuv422p"), ("DNxHR HQ", "dnxhr_hq", "yuv422p"), ("DNxHR HQX 10-bit", "dnxhr_hqx", "yuv422p10le"), ("DNxHR 444", "dnxhr_444", "yuv444p10le")] {
        let p = b.add(e, &format!("Avid / Resolve – {} (MOV)", name), "mov", V::DnxHr, A::Pcm16, 0, 0, 0.0, Best, 0, "");
        p.extra = s(&["-profile:v", prof, "-pix_fmt", pix]);
    }
    for (name, prof, pix) in [("DNxHR HQ", "dnxhr_hq", "yuv422p")] {
        let p = b.add(e, &format!("Avid / Resolve – {} (MXF)", name), "mxf", V::DnxHr, A::Pcm16, 0, 0, 0.0, Best, 0, "");
        p.extra = s(&["-profile:v", prof, "-pix_fmt", pix]);
    }
    b.add(e, "Adobe Premiere / Sony Vegas – MPEG-2 (MPG)", "mpg", V::Mpeg2, A::Pcm16, 0, 0, 0.0, Best, 0, "");
    b.add(e, "Adobe Premiere / Sony Vegas – H.264 intra (MOV)", "mov", V::H264, A::Pcm16, 0, 0, 0.0, Best, 0, "All-keyframe H.264: smooth scrubbing.").extra = s(&["-g", "1"]);
    b.add(e, "Adobe Premiere – MP4 H.264 (edit friendly)", "mp4", V::H264, A::Aac, 0, 0, 0.0, Best, 320, "").extra = s(&["-g", "15", "-bf", "0"]);
    b.add(e, "Avid / iMovie – Motion JPEG (MOV)", "mov", V::Mjpeg, A::Pcm16, 0, 0, 0.0, Best, 0, "");
    b.add(e, "DaVinci Resolve (free) – H.265 intra 10-bit", "mov", V::H265, A::Pcm24, 0, 0, 0.0, Best, 0, "").ten_bit = true;
    b.add(e, "Proxy files – 540p H.264", "mov", V::H264, A::Aac, 0, 540, 0.0, Medium, 128, "Light copies for smooth editing on slow machines.").extra = s(&["-g", "1"]);

    // ---------------- Lossless & archive ----------------
    let l = "Lossless & Archive";
    b.add(l, "FFV1 lossless – MKV", "mkv", V::Ffv1, A::Flac, 0, 0, 0.0, Best, 0, "Mathematically lossless, for preservation.").extra = s(&["-level", "3", "-g", "1", "-slicecrc", "1"]);
    b.add(l, "HuffYUV lossless – AVI", "avi", V::Huffyuv, A::Pcm16, 0, 0, 0.0, Best, 0, "");
    b.add(l, "H.264 lossless – MKV", "mkv", V::H264, A::Flac, 0, 0, 0.0, Best, 0, "").extra = s(&["-qp", "0"]);
    b.add(l, "H.265 lossless – MKV", "mkv", V::H265, A::Flac, 0, 0, 0.0, Best, 0, "").extra = s(&["-x265-params", "lossless=1"]);
    b.add(l, "Uncompressed raw video – AVI", "avi", V::Huffyuv, A::Pcm16, 0, 0, 0.0, Best, 0, "Very large files.").extra = s(&["-c:v", "rawvideo", "-pix_fmt", "yuv420p"]);
    b.add(l, "Archive – H.265 10-bit near-lossless", "mkv", V::H265, A::Flac, 0, 0, 0.0, Best, 0, "").ten_bit = true;
    b.add(l, "Archive – AV1 10-bit near-lossless", "mkv", V::Av1, A::Flac, 0, 0, 0.0, Best, 0, "").ten_bit = true;

    // ---------------- Remux ----------------
    let rm = "Remux (no quality loss)";
    for (ext, name) in [("mkv", "MKV"), ("mp4", "MP4"), ("mov", "MOV"), ("ts", "TS"), ("m2ts", "M2TS"), ("avi", "AVI"), ("webm", "WebM"), ("flv", "FLV")] {
        b.add(rm, &format!("Copy all streams into {}", name), ext, V::Copy, A::Copy, 0, 0, 0.0, High, 0, "No re-encoding: instant, no quality loss. The streams must be supported by the new container.");
    }
    b.add(rm, "Keep video, convert sound to AAC – MP4", "mp4", V::Copy, A::Aac, 0, 0, 0.0, High, 192, "Fixes 'no sound' on TVs that can't play DTS/AC-3.");
    b.add(rm, "Keep video, convert sound to AC-3 – MKV", "mkv", V::Copy, A::Ac3, 0, 0, 0.0, High, 448, "");
    b.add(rm, "Extract audio track (original format) – MKA", "mka", V::None, A::Copy, 0, 0, 0.0, High, 0, "");
    b.add(rm, "Extract video only (no sound) – MKV", "mkv", V::Copy, A::None, 0, 0, 0.0, High, 0, "");

    // ---------------- 3D ----------------
    let d = "3D Video";
    for (mode, mname) in [(Stereo::AnaglyphFromSbs, "Anaglyph red/cyan (from side-by-side)"), (Stereo::AnaglyphFromTb, "Anaglyph red/cyan (from top-bottom)"), (Stereo::SbsToTb, "Top-bottom (from side-by-side)"), (Stereo::TbToSbs, "Side-by-side (from top-bottom)"), (Stereo::SbsHalf, "Half side-by-side (from full SBS)")] {
        for (ext, ename) in [("mp4", "MP4"), ("mkv", "MKV"), ("m2ts", "M2TS")] {
            let p = b.add(d, &format!("3D {} – {}", mname, ename), ext, V::H264, A::Aac, 0, 0, 0.0, High, 192, "For 3D TVs, VR headsets and red/cyan glasses.");
            p.stereo = mode;
        }
    }

    // ---------------- DVD / VCD / Blu-ray ----------------
    let dv = "DVD, VCD & Blu-ray";
    for (name, target) in [("DVD video – NTSC", "ntsc-dvd"), ("DVD video – PAL", "pal-dvd"), ("VCD – NTSC", "ntsc-vcd"), ("VCD – PAL", "pal-vcd"), ("SVCD – NTSC", "ntsc-svcd"), ("SVCD – PAL", "pal-svcd")] {
        let p = b.add(dv, name, "mpg", V::Mpeg2, A::Ac3, 0, 0, 0.0, High, 0, "Ready to burn with any disc authoring tool.");
        p.extra = s(&["-target", target]);
        p.channels = 0;
        if target.contains("vcd") && !target.contains("svcd") {
            p.vcodec = V::Mpeg1;
            p.acodec = A::Mp2;
        }
        if target.contains("svcd") {
            p.acodec = A::Mp2;
        }
    }
    b.add(dv, "DVD video 16:9 – NTSC", "mpg", V::Mpeg2, A::Ac3, 0, 0, 0.0, High, 0, "").extra = s(&["-target", "ntsc-dvd", "-aspect", "16:9"]);
    b.add(dv, "DVD video 16:9 – PAL", "mpg", V::Mpeg2, A::Ac3, 0, 0, 0.0, High, 0, "").extra = s(&["-target", "pal-dvd", "-aspect", "16:9"]);
    b.add(dv, "Blu-ray compatible – 1080p M2TS", "m2ts", V::H264, A::Ac3, 1920, 1080, 0.0, Best, 640, "").extra = s(&["-profile:v", "high", "-level", "4.1", "-bluray-compat", "1"]);
    b.add(dv, "AVCHD – 1080p M2TS", "m2ts", V::H264, A::Ac3, 1920, 1080, 0.0, High, 384, "").extra = s(&["-level", "4.0"]);
    b.add(dv, "AVCHD Lite – 720p M2TS", "m2ts", V::H264, A::Ac3, 1280, 720, 0.0, High, 256, "");

    // ---------------- DV & Interlaced ----------------
    let di = "DV & Interlaced";
    b.add(di, "DV – NTSC (AVI)", "avi", V::Dv, A::Pcm16, 720, 480, 29.97, Best, 0, "").extra = s(&["-pix_fmt", "yuv411p"]);
    b.add(di, "DV – PAL (AVI)", "avi", V::Dv, A::Pcm16, 720, 576, 25.0, Best, 0, "").extra = s(&["-pix_fmt", "yuv420p"]);
    b.add(di, "DV – NTSC (MOV)", "mov", V::Dv, A::Pcm16, 720, 480, 29.97, Best, 0, "").extra = s(&["-pix_fmt", "yuv411p"]);
    b.add(di, "DV – PAL (MOV)", "mov", V::Dv, A::Pcm16, 720, 576, 25.0, Best, 0, "").extra = s(&["-pix_fmt", "yuv420p"]);
    b.add(di, "Interlaced 1080i – H.264 (TS)", "ts", V::H264, A::Ac3, 1920, 1080, 29.97, High, 384, "For broadcast-style playback.").extra = s(&["-flags", "+ilme+ildct", "-x264opts", "tff=1"]);
    b.add(di, "Interlaced 576i – MPEG-2 PAL", "mpg", V::Mpeg2, A::Mp2, 720, 576, 25.0, High, 224, "").extra = s(&["-flags", "+ilme+ildct", "-top", "1"]);
    b.add(di, "Interlaced 480i – MPEG-2 NTSC", "mpg", V::Mpeg2, A::Mp2, 720, 480, 29.97, High, 224, "").extra = s(&["-flags", "+ilme+ildct", "-top", "1"]);

    // ---------------- Windows & Office ----------------
    let wi = "Windows & Office";
    b.add(wi, "PowerPoint – MP4 1080p", "mp4", V::H264, A::Aac, 0, 1080, 0.0, High, 160, "Embeds cleanly in presentations.");
    b.add(wi, "PowerPoint – MP4 720p", "mp4", V::H264, A::Aac, 0, 720, 0.0, Medium, 128, "");
    b.add(wi, "PowerPoint (older) – WMV", "wmv", V::Wmv2, A::Wma, 0, 720, 0.0, High, 160, "");
    b.add(wi, "Windows Media Player – WMV HD", "wmv", V::Wmv2, A::Wma, 0, 1080, 0.0, High, 192, "");
    b.add(wi, "Windows Photos / Clipchamp – MP4", "mp4", V::H264, A::Aac, 0, 0, 0.0, High, 160, "");
    b.add(wi, "Teams / Zoom recording – small MP4", "mp4", V::H264, A::Aac, 0, 720, 15.0, Low, 64, "Screen recordings stay sharp at low size.");
    b.add(wi, "Screen recording – lossless-ish H.264", "mkv", V::H264, A::Aac, 0, 0, 0.0, Best, 160, "");

    // ---------------- Older phones & players ----------------
    let o = "Older Phones & Players";
    for (name, ext, codec, ac, wd, ht, fps) in [
        ("Feature phone – 3GP (176×144)", "3gp", V::H263, A::AmrNb, 176u32, 144u32, 15.0f32),
        ("Feature phone – 3GP (352×288)", "3gp", V::H263, A::AmrNb, 352, 288, 15.0),
        ("Old smartphone – MP4 (320×240)", "mp4", V::Mpeg4, A::Aac, 320, 240, 24.0),
        ("Old smartphone – MP4 (480×320)", "mp4", V::H264, A::Aac, 480, 320, 30.0),
        ("Pocket media player – AVI (320×240)", "avi", V::Xvid, A::Mp3, 320, 240, 24.0),
        ("Pocket media player – WMV (320×240)", "wmv", V::Wmv2, A::Wma, 320, 240, 24.0),
        ("Portable player – MP4 (480×272)", "mp4", V::H264, A::Aac, 480, 272, 30.0),
        ("Portable player – AVI (640×480)", "avi", V::Xvid, A::Mp3, 640, 480, 30.0),
        ("Tablet (first generation) – 1024×600", "mp4", V::H264, A::Aac, 1024, 600, 30.0),
        ("Netbook – 1024×576", "mp4", V::H264, A::Aac, 1024, 576, 30.0),
    ] {
        let p = b.add(o, name, ext, codec, ac, wd, ht, fps, Medium, if ac == A::AmrNb { 12 } else { 96 }, "");
        if ac == A::AmrNb {
            p.sample_rate = 8000;
            p.channels = 1;
        }
        if codec == V::H264 {
            p.extra = s(&["-profile:v", "baseline"]);
        }
    }

    // ---------------- Flash & legacy web ----------------
    let f = "Flash & Legacy Web";
    b.add(f, "FLV – H.264 (720p)", "flv", V::H264, A::Aac, 0, 720, 0.0, High, 128, "");
    b.add(f, "FLV – H.264 (480p)", "flv", V::H264, A::Aac, 0, 480, 0.0, Medium, 96, "");
    b.add(f, "FLV – Sorenson Spark (old players)", "flv", V::Flv1, A::Mp3, 0, 480, 0.0, Medium, 96, "").sample_rate = 44100;
    b.add(f, "F4V – H.264", "f4v", V::H264, A::Aac, 0, 0, 0.0, High, 128, "");
    b.add(f, "OGG Theora video", "ogv", V::Theora, A::Vorbis, 0, 720, 0.0, Medium, 128, "");

    // ---------------- Audio for devices ----------------
    let ad = "Audio for Devices";
    b.audio(ad, "Audiobook (M4B, mono)", "m4b", A::Aac, 64, 44100, 1, "Remembers the position in book players.");
    b.audio(ad, "Podcast – MP3 mono 64 kbps", "mp3", A::Mp3, 64, 44100, 1, "");
    b.audio(ad, "Podcast – MP3 stereo 128 kbps", "mp3", A::Mp3, 128, 44100, 2, "");
    b.audio(ad, "Voice memo – AAC mono 48 kbps", "m4a", A::Aac, 48, 22050, 1, "");
    b.audio(ad, "Car stereo – MP3 192 kbps", "mp3", A::Mp3, 192, 44100, 2, "");
    b.audio(ad, "Android ringtone – OGG", "ogg", A::Vorbis, 160, 44100, 2, "");
    b.audio(ad, "Smart speaker – AAC 128 kbps", "m4a", A::Aac, 128, 44100, 2, "");
    b.audio(ad, "CD quality – WAV 44.1 kHz", "wav", A::Pcm16, 0, 44100, 2, "");
    b.audio(ad, "Hi-Res – FLAC 24-bit / 192 kHz", "flac", A::Flac, 0, 192000, 2, "");
    b.audio(ad, "Home theatre – AC-3 5.1", "ac3", A::Ac3, 640, 48000, 6, "");
    b.audio(ad, "Game / app sound – OGG 96 kbps", "ogg", A::Vorbis, 96, 44100, 2, "");
    b.audio(ad, "Telephone / IVR – WAV 8 kHz mono", "wav", A::Pcm16, 0, 8000, 1, "");

    // ---------------- more variety ----------------
    for (q, ql) in [(Best, "best quality"), (Medium, "balanced"), (Low, "small file")] {
        b.add(c, &format!("MP4 – H.264 ({})", ql), "mp4", V::H264, A::Aac, 0, 0, 0.0, q, 160, "");
        b.add(c, &format!("MP4 – H.265 ({})", ql), "mp4", V::H265, A::Aac, 0, 0, 0.0, q, 160, "");
        b.add(c, &format!("MKV – H.265 ({})", ql), "mkv", V::H265, A::Aac, 0, 0, 0.0, q, 192, "");
    }
    for height in [720u32, 1080, 2160] {
        b.add(h, &format!("{} H.264 – MOV", res_name(height)), "mov", V::H264, A::Aac, 0, height, 0.0, High, 192, "");
        b.add(h, &format!("{} H.265 – MOV", res_name(height)), "mov", V::H265, A::Aac, 0, height, 0.0, High, 192, "");
        b.add(h, &format!("{} 50 fps H.264 – MP4 (PAL regions)", res_name(height)), "mp4", V::H264, A::Aac, 0, height, 50.0, High, 192, "");
        b.add(h, &format!("{} H.264 – TS", res_name(height)), "ts", V::H264, A::Ac3, 0, height, 0.0, High, 384, "");
    }
    b.add(h, "480p SD VP9 – WebM", "webm", V::Vp9, A::Opus, 0, 480, 0.0, High, 128, "");
    b.add(h, "8K AV1 60 fps – MKV", "mkv", V::Av1, A::Opus, 0, 4320, 60.0, High, 256, "");
    b.add(h, "4K 120 fps H.265 – MP4", "mp4", V::H265, A::Aac, 0, 2160, 120.0, High, 256, "");

    for k in [64u32, 32] {
        b.audio(a, &format!("MP3 – {} kbps (voice)", k), "mp3", A::Mp3, k, if k == 32 { 22050 } else { 44100 }, 1, "");
    }
    for k in [96u32, 64] {
        b.audio(a, &format!("AAC (M4A) – {} kbps", k), "m4a", A::Aac, k, 44100, 2, "");
    }
    b.audio(a, "AAC – raw ADTS (.aac)", "aac", A::Aac, 192, 44100, 2, "");
    b.audio(a, "FLAC – 16-bit / 48 kHz", "flac", A::Flac, 0, 48000, 2, "");
    b.audio(a, "WAV – 24-bit / 96 kHz", "wav", A::Pcm24, 0, 96000, 2, "");
    b.audio(a, "ALAC – 24-bit / 96 kHz", "m4a", A::Alac, 0, 96000, 2, "");
    for k in [32u32, 24] {
        b.audio(a, &format!("Opus – {} kbps (speech)", k), "opus", A::Opus, k, 48000, 1, "");
    }
    for k in [320u32, 96, 64] {
        b.audio(a, &format!("OGG Vorbis – {} kbps", k), "ogg", A::Vorbis, k, 44100, 2, "");
    }
    b.audio(a, "WMA – 128 kbps", "wma", A::Wma, 128, 44100, 2, "");
    b.audio(a, "WMA – 64 kbps", "wma", A::Wma, 64, 44100, 2, "");
    b.audio(a, "MKA – Matroska audio (FLAC)", "mka", A::Flac, 0, 0, 0, "");

    b.add(w, "Snapchat (1080×1920)", "mp4", V::H264, A::Aac, 1080, 1920, 30.0, High, 128, "");
    b.add(w, "Pinterest (1000×1500)", "mp4", V::H264, A::Aac, 1000, 1500, 30.0, High, 128, "");
    b.add(w, "Reddit (1080p)", "mp4", V::H264, A::Aac, 0, 1080, 30.0, High, 128, "");
    b.add(w, "Dailymotion (1080p)", "mp4", V::H264, A::Aac, 0, 1080, 0.0, High, 192, "");
    b.add(w, "Bilibili (1080p)", "mp4", V::H264, A::Aac, 0, 1080, 0.0, High, 192, "");
    b.add(w, "Threads / Bluesky (1080×1920)", "mp4", V::H264, A::Aac, 1080, 1920, 30.0, High, 128, "");
    b.add(w, "YouTube 8K (AV1)", "mp4", V::Av1, A::Opus, 0, 4320, 0.0, High, 256, "");

    for (name, codec, height) in [
        ("Xiaomi / Redmi phone – 1080p", V::H264, 1080u32),
        ("OnePlus phone – 1080p HEVC", V::H265, 1080),
        ("Motorola phone – 1080p", V::H264, 1080),
        ("Sony Xperia – 4K HEVC", V::H265, 2160),
        ("Huawei / Honor phone – 1080p", V::H264, 1080),
        ("OPPO / Vivo / Realme phone – 1080p", V::H264, 1080),
        ("Nokia Android phone – 720p", V::H264, 720),
        ("Kindle Fire tablet – 1080p", V::H264, 1080),
        ("Galaxy Tab – 1600p HEVC", V::H265, 1600),
        ("Android TV box – 4K HEVC", V::H265, 2160),
    ] {
        b.add(an, name, "mp4", codec, A::Aac, 0, height, 0.0, High, 160, "");
    }
    b.add(ap, "iPhone – vertical video (1080×1920)", "m4v", V::H264, A::Aac, 1080, 1920, 30.0, High, 160, "");
    b.add(ap, "iPhone – 4K 60 fps HEVC", "m4v", V::H265, A::Aac, 0, 2160, 60.0, High, 192, "").extra = s(&["-tag:v", "hvc1"]);
    b.add(ap, "Apple Watch (small 368×448)", "m4v", V::H264, A::Aac, 368, 448, 30.0, Medium, 96, "");

    for (name, codec, height, ac, k) in [
        ("Samsung TV – 4K HEVC", V::H265, 2160u32, A::Eac3, 640u32),
        ("Samsung TV – 1080p", V::H264, 1080, A::Ac3, 448),
        ("LG TV – 4K HEVC", V::H265, 2160, A::Eac3, 640),
        ("LG TV – 1080p", V::H264, 1080, A::Ac3, 448),
        ("Sony / Philips TV – 4K HEVC", V::H265, 2160, A::Eac3, 640),
        ("Sony / Philips TV – 1080p", V::H264, 1080, A::Ac3, 448),
        ("Hisense / TCL TV – 4K HEVC", V::H265, 2160, A::Eac3, 640),
        ("Older HD TV (USB) – 720p", V::H264, 720, A::Aac, 160),
    ] {
        b.add(tv, name, "mkv", codec, ac, 0, height, 0.0, High, k, "");
    }

    let cd = "Cameras & Drones";
    b.add(cd, "Action camera 4K – shrink to HEVC", "mp4", V::H265, A::Aac, 0, 2160, 0.0, High, 192, "Same picture, much smaller files.");
    b.add(cd, "Action camera – 1080p for sharing", "mp4", V::H264, A::Aac, 0, 1080, 0.0, High, 160, "");
    b.add(cd, "Drone 4K – archive HEVC 10-bit", "mkv", V::H265, A::Aac, 0, 2160, 0.0, High, 160, "").ten_bit = true;
    b.add(cd, "Drone – 1080p for sharing", "mp4", V::H264, A::Aac, 0, 1080, 0.0, High, 128, "");
    b.add(cd, "Dashcam – compress (720p, small)", "mp4", V::H265, A::Aac, 0, 720, 0.0, Low, 64, "");
    b.add(cd, "Security camera – long-term storage", "mkv", V::H265, A::Aac, 0, 720, 15.0, Low, 48, "");
    b.add(cd, "Webcam recording – clean up (720p)", "mp4", V::H264, A::Aac, 0, 720, 30.0, Medium, 96, "");
    b.add(cd, "Camcorder AVCHD (MTS) – MP4", "mp4", V::H264, A::Aac, 0, 0, 0.0, High, 192, "");
    b.add(cd, "Old camcorder tape (DV) – MP4", "mp4", V::H264, A::Aac, 0, 0, 0.0, High, 192, "Turn on Deinterlace in Edit for best results.");
    b.add(cd, "Slow-motion clip – keep frame rate (1080p HEVC)", "mp4", V::H265, A::Aac, 0, 1080, 0.0, High, 160, "");

    for (name, height, codec) in [
        ("360° video – 4K", 2160u32, V::H265),
        ("360° video – 5.7K", 2880, V::H265),
        ("360° video – 8K", 4320, V::H265),
        ("360° video – 4K (H.264, wide support)", 2160, V::H264),
        ("VR180 side-by-side – 4K", 2160, V::H265),
        ("VR headset – 2880p HEVC", 2880, V::H265),
    ] {
        b.add(d, name, "mp4", codec, A::Aac, 0, height, 0.0, High, 256, "For VR headsets and 360° players.");
    }

    b.list
}

/// Same profile, same id across versions (FNV-1a of category + name).
fn stable_id(cat: &str, name: &str) -> u32 {
    let mut h: u32 = 0x811c9dc5;
    for b in cat.bytes().chain([0u8]).chain(name.bytes()) {
        h ^= b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    1_000_000 + h % 8_000_000
}

fn s(v: &[&str]) -> Vec<String> {
    v.iter().map(|x| x.to_string()).collect()
}

pub fn category_icon(cat: &str) -> &'static str {
    CATEGORIES.iter().find(|(c, _)| *c == cat).map(|(_, i)| *i).unwrap_or("•")
}
