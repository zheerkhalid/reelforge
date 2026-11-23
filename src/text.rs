//! Fonts for scripts the built-in egui font lacks (Arabic script, Indic, CJK).
//!
//! These are fallbacks for text egui draws itself (and for typing). Names and
//! other user text with right-to-left letters are drawn by Windows instead:
//! see `bidi.rs`.

use eframe::egui;

/// Add system fonts with Arabic, Indic and CJK letters as fallbacks.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let mut names = Vec::new();
    let candidates: &[(&str, &str, u32)] = if cfg!(windows) {
        &[
            ("segoe", "C:\\Windows\\Fonts\\segoeui.ttf", 0),
            ("tahoma", "C:\\Windows\\Fonts\\tahoma.ttf", 0),
            ("arial", "C:\\Windows\\Fonts\\arial.ttf", 0),
            ("nirmala", "C:\\Windows\\Fonts\\Nirmala.ttc", 0),
            ("yahei", "C:\\Windows\\Fonts\\msyh.ttc", 0),
            ("malgun", "C:\\Windows\\Fonts\\malgun.ttf", 0),
        ]
    } else if cfg!(target_os = "macos") {
        &[("geeza", "/System/Library/Fonts/GeezaPro.ttc", 0), ("arial", "/System/Library/Fonts/Supplemental/Arial Unicode.ttf", 0), ("pingfang", "/System/Library/Fonts/PingFang.ttc", 0)]
    } else {
        &[
            ("noto-arabic", "/usr/share/fonts/truetype/noto/NotoSansArabic-Regular.ttf", 0),
            ("noto-naskh", "/usr/share/fonts/truetype/noto/NotoNaskhArabic-Regular.ttf", 0),
            ("dejavu", "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf", 0),
        ]
    };
    for (name, path, index) in candidates {
        if let Ok(bytes) = std::fs::read(path) {
            let mut data = egui::FontData::from_owned(bytes);
            data.index = *index;
            fonts.font_data.insert(name.to_string(), std::sync::Arc::new(data));
            names.push(name.to_string());
        }
    }
    for family in [egui::FontFamily::Proportional, egui::FontFamily::Monospace] {
        if let Some(list) = fonts.families.get_mut(&family) {
            list.extend(names.iter().cloned());
        }
    }
    ctx.set_fonts(fonts);
}
