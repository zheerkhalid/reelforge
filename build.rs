//! Build script: renders the ReelForge icon (vector, via tiny-skia) into a
//! multi-size .ico and a PNG for the window, then embeds the icon and version info.

#![allow(clippy::field_reassign_with_default)]

use std::{env, fs, path::PathBuf};
use tiny_skia::*;

fn rounded_rect(x0: f32, y0: f32, x1: f32, y1: f32, r: f32) -> Path {
    let mut pb = PathBuilder::new();
    pb.move_to(x0 + r, y0);
    pb.line_to(x1 - r, y0);
    pb.quad_to(x1, y0, x1, y0 + r);
    pb.line_to(x1, y1 - r);
    pb.quad_to(x1, y1, x1 - r, y1);
    pb.line_to(x0 + r, y1);
    pb.quad_to(x0, y1, x0, y1 - r);
    pb.line_to(x0, y0 + r);
    pb.quad_to(x0, y0, x0 + r, y0);
    pb.close();
    pb.finish().expect("rounded rect")
}

fn render_icon(size: u32) -> Vec<u8> {
    let mut pm = Pixmap::new(size, size).expect("pixmap");
    let s = size as f32 / 256.0;
    let id = Transform::identity();

    // Background: rounded square, coral -> magenta gradient.
    let (x0, y0, x1, y1) = (8.0 * s, 8.0 * s, 248.0 * s, 248.0 * s);
    let bg = rounded_rect(x0, y0, x1, y1, 56.0 * s);
    let mut paint = Paint::default();
    paint.anti_alias = true;
    paint.shader = LinearGradient::new(
        Point::from_xy(x0, y0),
        Point::from_xy(x1, y1),
        vec![GradientStop::new(0.0, Color::from_rgba8(0xff, 0x8a, 0x3d, 255)), GradientStop::new(1.0, Color::from_rgba8(0xe0, 0x2f, 0x7a, 255))],
        SpreadMode::Pad,
        id,
    )
    .expect("gradient");
    pm.fill_path(&bg, &paint, FillRule::Winding, id, None);

    // Film-strip perforations along the top and bottom (skipped at tiny sizes).
    if size >= 32 {
        let mut holes = Paint::default();
        holes.anti_alias = true;
        holes.set_color_rgba8(255, 255, 255, 110);
        for row in [34.0f32, 204.0] {
            for i in 0..5 {
                let hx = 48.0 + i as f32 * 36.0;
                let p = rounded_rect(hx * s, row * s, (hx + 18.0) * s, (row + 18.0) * s, 5.0 * s);
                pm.fill_path(&p, &holes, FillRule::Winding, id, None);
            }
        }
    }

    // White play triangle with soft rounded corners.
    let mut white = Paint::default();
    white.anti_alias = true;
    white.set_color_rgba8(255, 255, 255, 255);
    let mut p = PathBuilder::new();
    p.move_to(98.0 * s, 78.0 * s);
    p.line_to(98.0 * s, 178.0 * s);
    p.line_to(184.0 * s, 128.0 * s);
    p.close();
    let tri = p.finish().expect("triangle");
    pm.fill_path(&tri, &white, FillRule::Winding, id, None);
    let stroke = Stroke { width: 18.0 * s, line_join: LineJoin::Round, ..Stroke::default() };
    pm.stroke_path(&tri, &white, &stroke, id, None);

    // Amber lightning bolt (GPU speed) on the lower right.
    let mut bolt = Paint::default();
    bolt.anti_alias = true;
    bolt.set_color_rgba8(0xff, 0xd2, 0x3f, 255);
    let mut b = PathBuilder::new();
    for (i, (x, y)) in [(196.0, 120.0), (160.0, 182.0), (184.0, 182.0), (170.0, 232.0), (214.0, 162.0), (190.0, 162.0), (204.0, 120.0)].iter().enumerate() {
        if i == 0 {
            b.move_to(x * s, y * s)
        } else {
            b.line_to(x * s, y * s)
        }
    }
    b.close();
    let bolt_path = b.finish().expect("bolt");
    let mut outline = Paint::default();
    outline.anti_alias = true;
    outline.set_color_rgba8(0x9a, 0x1b, 0x50, 255);
    pm.stroke_path(&bolt_path, &outline, &Stroke { width: 8.0 * s, line_join: LineJoin::Round, ..Stroke::default() }, id, None);
    pm.fill_path(&bolt_path, &bolt, FillRule::Winding, id, None);

    pm.encode_png().expect("png")
}

fn write_ico(path: &PathBuf, sizes: &[u32]) {
    let pngs: Vec<Vec<u8>> = sizes.iter().map(|&s| render_icon(s)).collect();
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]);
    out.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * sizes.len() as u32;
    for (i, &s) in sizes.iter().enumerate() {
        let b = if s >= 256 { 0 } else { s as u8 };
        out.extend_from_slice(&[b, b, 0, 0, 1, 0, 32, 0]);
        out.extend_from_slice(&(pngs[i].len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += pngs[i].len() as u32;
    }
    for p in &pngs {
        out.extend_from_slice(p);
    }
    fs::write(path, out).expect("write ico");
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR"));
    let ico = out.join("reelforge.ico");
    write_ico(&ico, &[16, 20, 24, 32, 40, 48, 64, 256]);
    let png = render_icon(256);
    fs::write(out.join("icon-256.png"), &png).expect("write png");
    let docs_png = PathBuf::from("docs/assets/reelforge-icon.png");
    if fs::read(&docs_png).ok().as_deref() != Some(&png[..]) {
        let _ = fs::create_dir_all("docs/assets");
        let _ = fs::write(&docs_png, &png);
    }

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let ver = env::var("CARGO_PKG_VERSION").expect("version");
    let parts: Vec<u16> = ver.split(['.', '-']).take(3).map(|p| p.parse().unwrap_or(0)).collect();
    let (ma, mi, pa) = (parts[0], parts.get(1).copied().unwrap_or(0), parts.get(2).copied().unwrap_or(0));
    let esc = |p: &PathBuf| p.display().to_string().replace('\\', "\\\\");
    let rc = format!(
        r#"#pragma code_page(65001)
1 ICON "{ico}"
1 VERSIONINFO
FILEVERSION {ma},{mi},{pa},0
PRODUCTVERSION {ma},{mi},{pa},0
FILEOS 0x40004
FILETYPE 0x1
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "CompanyName", "Zheer Khalid (zheer.dev)"
      VALUE "FileDescription", "ReelForge video and audio converter"
      VALUE "FileVersion", "{ver}"
      VALUE "InternalName", "ReelForge"
      VALUE "LegalCopyright", "Copyright (c) 2026 Zheer Khalid. MIT License."
      VALUE "OriginalFilename", "ReelForge.exe"
      VALUE "ProductName", "ReelForge"
      VALUE "ProductVersion", "{ver}"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        ico = esc(&ico),
    );
    let rc_path = out.join("reelforge.rc");
    fs::write(&rc_path, rc).expect("write rc");
    embed_resource::compile(&rc_path, embed_resource::NONE);
}
