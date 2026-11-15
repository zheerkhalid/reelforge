//! Text that egui can't draw on its own: Arabic-script names (Kurdish Sorani,
//! Arabic, Persian…) and other scripts the built-in font lacks.
//!
//! egui draws one glyph per character, left to right, without joining. So we
//! (1) load system fonts that contain those letters, and (2) turn the letters
//! into their joined presentation forms and put right-to-left runs in visual
//! order before handing the text to egui.

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
        &[
            ("geeza", "/System/Library/Fonts/GeezaPro.ttc", 0),
            ("arial", "/System/Library/Fonts/Supplemental/Arial Unicode.ttf", 0),
            ("pingfang", "/System/Library/Fonts/PingFang.ttc", 0),
        ]
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

fn is_rtl_char(c: char) -> bool {
    matches!(c as u32, 0x0590..=0x08FF | 0xFB1D..=0xFDFF | 0xFE70..=0xFEFF)
}

/// Make a string display correctly: join Arabic-script letters and order
/// right-to-left words. Text without such letters is returned unchanged.
pub fn shape(s: &str) -> String {
    if !s.chars().any(is_rtl_char) {
        return s.to_string();
    }
    let joined = ar_reshaper::reshape_line(s);
    let bidi = unicode_bidi::BidiInfo::new(&joined, Some(unicode_bidi::Level::ltr()));
    let mut out = String::with_capacity(joined.len());
    for para in &bidi.paragraphs {
        let line = para.range.clone();
        out.push_str(&bidi.reorder_line(para, line));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn latin_untouched() {
        assert_eq!(shape("Hello.mp4"), "Hello.mp4");
    }
    #[test]
    fn kurdish_reordered() {
        let s = shape("001 - باران.mp4");
        assert!(s.starts_with("001 - "));
        assert!(s.ends_with(".mp4"));
        assert!(!s.contains('\u{0628}'), "letters should be in joined forms: {}", s);
    }
}
