//! Graphics-card detection.
//!
//! We list the adapters Windows reports and then *prove* each hardware
//! encoder works with a tiny test encode, so the app only offers what runs.

use crate::engine::{ffmpeg_path, hidden_command};
use crate::profiles::VCodec;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize, Hash)]
pub enum Vendor {
    Nvidia,
    Amd,
    Intel,
    Other,
}

impl Vendor {
    pub fn label(self) -> &'static str {
        match self {
            Vendor::Nvidia => "NVIDIA (NVENC)",
            Vendor::Amd => "AMD (AMF)",
            Vendor::Intel => "Intel (Quick Sync)",
            Vendor::Other => "Other",
        }
    }
    pub fn short(self) -> &'static str {
        match self {
            Vendor::Nvidia => "NVIDIA",
            Vendor::Amd => "AMD",
            Vendor::Intel => "Intel",
            Vendor::Other => "GPU",
        }
    }
    /// Discrete / external cards first: NVIDIA, then AMD, then Intel.
    fn rank(self) -> u8 {
        match self {
            Vendor::Nvidia => 0,
            Vendor::Amd => 1,
            Vendor::Intel => 2,
            Vendor::Other => 3,
        }
    }
    fn suffix(self) -> &'static str {
        match self {
            Vendor::Nvidia => "nvenc",
            Vendor::Amd => "amf",
            Vendor::Intel => "qsv",
            Vendor::Other => "",
        }
    }
    /// Decoder used for "GPU decode acceleration".
    pub fn hwaccel(self) -> &'static str {
        match self {
            Vendor::Nvidia => "cuda",
            Vendor::Amd => "d3d11va",
            Vendor::Intel => "d3d11va",
            Vendor::Other => "auto",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Adapter {
    pub name: String,
    pub vendor: Vendor,
    /// Probably a separate (external / discrete) card.
    pub discrete: bool,
}

#[derive(Clone, Debug)]
pub struct HwEncoder {
    pub vendor: Vendor,
    pub codec: VCodec,
    pub name: String,
}

#[derive(Clone, Debug, Default)]
pub struct GpuReport {
    pub done: bool,
    pub adapters: Vec<Adapter>,
    pub encoders: Vec<HwEncoder>,
}

/// Which hardware the person wants to use.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum AccelChoice {
    /// Best card found: an external/discrete card first.
    Auto,
    Vendor(Vendor),
    CpuOnly,
}

impl GpuReport {
    pub fn vendors(&self) -> Vec<Vendor> {
        let mut v: Vec<Vendor> = self.encoders.iter().map(|e| e.vendor).collect();
        v.sort_by_key(|x| x.rank());
        v.dedup();
        v
    }

    pub fn adapter_for(&self, vendor: Vendor) -> Option<&Adapter> {
        self.adapters.iter().find(|a| a.vendor == vendor)
    }

    /// Pick the hardware encoder for a codec according to the choice.
    pub fn pick(&self, codec: VCodec, choice: AccelChoice) -> Option<&HwEncoder> {
        if !codec.gpu_capable() {
            return None;
        }
        match choice {
            AccelChoice::CpuOnly => None,
            AccelChoice::Vendor(v) => self.encoders.iter().find(|e| e.vendor == v && e.codec == codec),
            AccelChoice::Auto => {
                let mut c: Vec<&HwEncoder> = self.encoders.iter().filter(|e| e.codec == codec).collect();
                // Prefer a discrete card, then vendor rank.
                c.sort_by_key(|e| {
                    let disc = self.adapter_for(e.vendor).map(|a| a.discrete).unwrap_or(false);
                    (!disc as u8, e.vendor.rank())
                });
                c.into_iter().next()
            }
        }
    }

    /// Vendor used for decoding, if any GPU is usable.
    pub fn decode_vendor(&self, choice: AccelChoice) -> Option<Vendor> {
        match choice {
            AccelChoice::CpuOnly => None,
            AccelChoice::Vendor(v) => self.vendors().into_iter().find(|x| *x == v).or_else(|| self.adapters.iter().map(|a| a.vendor).find(|x| *x == v)),
            AccelChoice::Auto => {
                self.pick(VCodec::H264, AccelChoice::Auto).map(|e| e.vendor).or_else(|| self.adapters.iter().map(|a| a.vendor).filter(|v| *v != Vendor::Other).min_by_key(|v| v.rank()))
            }
        }
    }

    pub fn headline(&self) -> String {
        if !self.done {
            return "Looking for graphics cards…".into();
        }
        match self.pick(VCodec::H264, AccelChoice::Auto) {
            Some(e) => {
                let name = self.adapter_for(e.vendor).map(|a| a.name.clone()).unwrap_or_else(|| e.vendor.short().into());
                format!("GPU acceleration ready: {}", name)
            }
            None => "No usable graphics card found – using the processor".into(),
        }
    }
}

fn classify(name: &str, compat: &str) -> Vendor {
    let n = format!("{} {}", name, compat).to_lowercase();
    if n.contains("nvidia") || n.contains("geforce") || n.contains("quadro") || n.contains("rtx") {
        Vendor::Nvidia
    } else if n.contains("amd") || n.contains("radeon") || n.contains("ati ") || n.contains("advanced micro") {
        Vendor::Amd
    } else if n.contains("intel") || n.contains("arc ") {
        Vendor::Intel
    } else {
        Vendor::Other
    }
}

fn list_adapters() -> Vec<Adapter> {
    let mut out = Vec::new();
    #[cfg(windows)]
    {
        let cmd = hidden_command("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", "Get-CimInstance Win32_VideoController | ForEach-Object { $_.Name + '|' + $_.AdapterCompatibility }"])
            .output();
        if let Ok(o) = cmd {
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                let mut parts = line.splitn(2, '|');
                let name = parts.next().unwrap_or("").trim().to_string();
                let compat = parts.next().unwrap_or("").trim().to_string();
                let vendor = classify(&name, &compat);
                let lower = name.to_lowercase();
                let discrete = match vendor {
                    Vendor::Nvidia => true,
                    Vendor::Amd => !(lower.contains("vega 8") || lower.contains("graphics") && !lower.contains("rx")),
                    Vendor::Intel => lower.contains("arc"),
                    Vendor::Other => false,
                };
                if lower.contains("basic display") || lower.contains("remote") {
                    continue;
                }
                out.push(Adapter { name, vendor, discrete });
            }
        }
    }
    #[cfg(not(windows))]
    {
        if let Ok(o) = hidden_command("sh").args(["-c", "lspci 2>/dev/null | grep -Ei 'vga|3d|display'"]).output() {
            for line in String::from_utf8_lossy(&o.stdout).lines() {
                let vendor = classify(line, "");
                out.push(Adapter { name: line.trim().to_string(), vendor, discrete: vendor != Vendor::Intel });
            }
        }
    }
    out
}

fn probe_encoder(ffmpeg: &std::path::Path, enc: &str) -> bool {
    let mut args: Vec<&str> = vec!["-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i", "color=c=black:s=320x240:r=30:d=0.2"];
    if enc.ends_with("_qsv") {
        args.extend(["-pix_fmt", "nv12"]);
    } else {
        args.extend(["-pix_fmt", "yuv420p"]);
    }
    args.extend(["-c:v", enc, "-f", "null", "-"]);
    hidden_command(ffmpeg).args(&args).output().map(|o| o.status.success()).unwrap_or(false)
}

/// Run detection in the background and fill the shared report.
pub fn detect_async(report: Arc<Mutex<GpuReport>>, ctx: eframe::egui::Context) {
    std::thread::spawn(move || {
        let adapters = list_adapters();
        let mut encoders = Vec::new();
        if let Some(ff) = ffmpeg_path() {
            let mut handles = Vec::new();
            for vendor in [Vendor::Nvidia, Vendor::Amd, Vendor::Intel] {
                for (codec, base) in [(VCodec::H264, "h264"), (VCodec::H265, "hevc"), (VCodec::Av1, "av1")] {
                    let name = format!("{}_{}", base, vendor.suffix());
                    let ff = ff.clone();
                    handles.push(std::thread::spawn(move || if probe_encoder(&ff, &name) { Some(HwEncoder { vendor, codec, name }) } else { None }));
                }
            }
            for h in handles {
                if let Ok(Some(e)) = h.join() {
                    encoders.push(e);
                }
            }
        }
        let mut r = report.lock().unwrap();
        r.adapters = adapters;
        // A working encoder proves the card exists even if Windows didn't name it.
        for e in &encoders {
            if !r.adapters.iter().any(|a| a.vendor == e.vendor) {
                r.adapters.push(Adapter { name: format!("{} graphics", e.vendor.short()), vendor: e.vendor, discrete: e.vendor != Vendor::Intel });
            }
        }
        r.encoders = encoders;
        r.done = true;
        drop(r);
        ctx.request_repaint();
    });
}
