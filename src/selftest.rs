//! `--selftest <folder>`: makes two short sample clips and converts them with
//! a spread of profiles (GPU and CPU, merge, split, edits), printing results.

use crate::engine::{self, Edits, JobInput, JobSpec, SubChoice};
use crate::gpu::{self, AccelChoice, GpuReport};
use crate::profiles::{self, Profile, VCodec};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

fn make_sample(ff: &Path, out: &Path, color: &str, secs: u32) {
    let _ = engine::hidden_command(ff)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i"])
        .arg(format!("testsrc2=s=1280x720:r=30:d={}", secs))
        .args(["-f", "lavfi", "-i"])
        .arg(format!("sine=frequency=440:d={}", secs))
        .args(["-vf", &format!("drawbox=c={}@0.3:t=fill", color), "-c:v", "libx264", "-pix_fmt", "yuv420p", "-c:a", "aac", "-shortest"])
        .arg(out)
        .status();
}

/// Write a minimal Blu-ray (PGS) picture subtitle file: a white box at the
/// bottom of a 1280×720 picture, shown from 0.5 s to 5.5 s.
fn write_pgs(path: &Path) {
    fn seg(out: &mut Vec<u8>, pts: u32, kind: u8, data: &[u8]) {
        out.extend_from_slice(b"PG");
        out.extend_from_slice(&pts.to_be_bytes());
        out.extend_from_slice(&0u32.to_be_bytes());
        out.push(kind);
        out.extend_from_slice(&(data.len() as u16).to_be_bytes());
        out.extend_from_slice(data);
    }
    let (vw, vh, bw, bh, x, y) = (1280u16, 720u16, 300u16, 50u16, 490u16, 640u16);
    let mut out = Vec::new();
    let on = 45_000u32; // 0.5 s at 90 kHz
    let off = 495_000u32; // 5.5 s
                          // Composition: epoch start, one object.
    let mut pcs = Vec::new();
    pcs.extend_from_slice(&vw.to_be_bytes());
    pcs.extend_from_slice(&vh.to_be_bytes());
    pcs.extend_from_slice(&[0x10, 0, 1, 0x80, 0, 0, 1, 0, 0, 0, 0]);
    pcs.extend_from_slice(&x.to_be_bytes());
    pcs.extend_from_slice(&y.to_be_bytes());
    seg(&mut out, on, 0x16, &pcs);
    let mut wds = vec![1u8, 0];
    for v in [x, y, bw, bh] {
        wds.extend_from_slice(&v.to_be_bytes());
    }
    seg(&mut out, on, 0x17, &wds);
    // Palette: entry 1 = opaque white.
    seg(&mut out, on, 0x14, &[0, 0, 1, 235, 128, 128, 255]);
    let mut rle = Vec::new();
    for _ in 0..bh {
        rle.extend_from_slice(&[0x00, 0xC0 | (bw >> 8) as u8, (bw & 0xFF) as u8, 0x01, 0x00, 0x00]);
    }
    let mut ods = vec![0u8, 0, 0, 0xC0];
    let len = (rle.len() + 4) as u32;
    ods.extend_from_slice(&len.to_be_bytes()[1..]);
    ods.extend_from_slice(&bw.to_be_bytes());
    ods.extend_from_slice(&bh.to_be_bytes());
    ods.extend_from_slice(&rle);
    seg(&mut out, on, 0x15, &ods);
    seg(&mut out, on, 0x80, &[]);
    // Clear.
    let mut pcs2 = Vec::new();
    pcs2.extend_from_slice(&vw.to_be_bytes());
    pcs2.extend_from_slice(&vh.to_be_bytes());
    pcs2.extend_from_slice(&[0x10, 0, 2, 0x00, 0, 0, 0]);
    seg(&mut out, off, 0x16, &pcs2);
    seg(&mut out, off, 0x17, &wds);
    seg(&mut out, off, 0x80, &[]);
    let _ = std::fs::write(path, out);
}

pub fn selftest(dir: PathBuf) {
    let _ = std::fs::create_dir_all(&dir);
    let Some(ff) = engine::ffmpeg_path() else {
        println!("FAIL: ffmpeg not found");
        return;
    };
    let a = dir.join("sample_a.mp4");
    let b = dir.join("sample_b.mp4");
    make_sample(&ff, &a, "red", 6);
    make_sample(&ff, &b, "blue", 4);
    let ctx = eframe::egui::Context::default();
    let rep = Arc::new(Mutex::new(GpuReport::default()));
    gpu::detect_async(rep.clone(), ctx.clone());
    while !rep.lock().unwrap().done {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let rep = rep.lock().unwrap().clone();
    println!("GPU: {}", rep.headline());
    for e in &rep.encoders {
        println!("  encoder {}", e.name);
    }
    let lib = profiles::library();
    println!("profiles: {}", lib.len());
    let info_a = engine::probe(&a);
    let info_b = engine::probe(&b);
    println!("probe a: ok={} {}x{} {:.2}s {} audio tracks", info_a.ok, info_a.width, info_a.height, info_a.duration, info_a.audio.len());
    let find = |n: &str| -> Profile { lib.iter().find(|p| p.name == n).unwrap_or_else(|| panic!("no profile {}", n)).clone() };
    let input =
        |p: &Path, info: &engine::MediaInfo, edits: Edits| JobInput { path: p.to_path_buf(), info: info.clone(), edits, audio_track: 0, subtitle: SubChoice::None, forced_only: false, sub_size: 0 };
    let edited = Edits { trim_start: 1.0, trim_end: 4.0, crop: [40, 20, 40, 20], watermark: "Sample: 1".into(), volume: 80.0, ..Default::default() };

    let mut cases: Vec<(String, JobSpec)> = Vec::new();
    let mut add = |label: &str, prof: Profile, inputs: Vec<JobInput>, accel: AccelChoice, split: f64| {
        let out = dir.join(format!("out_{}.{}", label, prof.ext));
        let _ = std::fs::remove_file(&out);
        let hw = rep.pick(prof.vcodec, accel).cloned();
        let dec = if prof.vcodec != VCodec::Copy && prof.vcodec != VCodec::None { rep.decode_vendor(accel) } else { None };
        cases.push((label.to_string(), JobSpec { inputs, profile: prof, output: out, hw_encoder: hw, hw_decode: dec, threads: 0, split_minutes: split }));
    };
    add("mp4_gpu", find("MP4 – works everywhere (H.264)"), vec![input(&a, &info_a, Edits::default())], AccelChoice::Auto, 0.0);
    add("hevc_gpu", find("MKV – H.265 / HEVC"), vec![input(&a, &info_a, Edits::default())], AccelChoice::Auto, 0.0);
    add("mp4_cpu_edits", find("MP4 – H.264"), vec![input(&a, &info_a, edited.clone())], AccelChoice::CpuOnly, 0.0);
    add("vertical", find("TikTok (1080×1920)"), vec![input(&a, &info_a, Edits::default())], AccelChoice::Auto, 0.0);
    add("mp3", find("MP3 – music (320 kbps)"), vec![input(&a, &info_a, Edits::default())], AccelChoice::Auto, 0.0);
    add("gif", find("Animated GIF (480 px)"), vec![input(&a, &info_a, edited.clone())], AccelChoice::Auto, 0.0);
    add("remux", find("Change container only (instant)"), vec![input(&a, &info_a, Edits::default())], AccelChoice::Auto, 0.0);
    add("webm", find("WebM – VP9"), vec![input(&b, &info_b, Edits::default())], AccelChoice::Auto, 0.0);
    add("prores", find("Final Cut Pro – Apple ProRes 422 (MOV)"), vec![input(&b, &info_b, Edits::default())], AccelChoice::Auto, 0.0);
    add("dvd", find("DVD video – PAL"), vec![input(&b, &info_b, Edits::default())], AccelChoice::Auto, 0.0);
    add("av1", find("MP4 – AV1"), vec![input(&b, &info_b, Edits::default())], AccelChoice::Auto, 0.0);
    add("merge", find("MP4 – works everywhere (H.264)"), vec![input(&a, &info_a, Edits::default()), input(&b, &info_b, Edits::default())], AccelChoice::Auto, 0.0);
    add("merge_copy", find("Copy all streams into MKV"), vec![input(&a, &info_a, Edits::default()), input(&b, &info_b, Edits::default())], AccelChoice::Auto, 0.0);
    add("split", find("MP4 – H.264"), vec![input(&a, &info_a, Edits::default())], AccelChoice::Auto, 0.05);
    add("anaglyph", find("3D Anaglyph red/cyan (from side-by-side) – MP4"), vec![input(&b, &info_b, Edits::default())], AccelChoice::Auto, 0.0);
    add("wmv", find("WMV – Windows Media (WMV8)"), vec![input(&b, &info_b, Edits::default())], AccelChoice::Auto, 0.0);
    add("3gp", find("3GP – H.263 / AMR"), vec![input(&b, &info_b, Edits::default())], AccelChoice::Auto, 0.0);

    // ---- subtitles: a text track, a picture (DVD) track and an old-encoding .srt
    let srt = dir.join("subs.srt");
    let _ = std::fs::write(&srt, "1\n00:00:00,500 --> 00:00:05,500\nHello subtitles\n\n");
    let ku = dir.join("kurdish_cp1256.srt");
    // "سڵاو" is not in CP1256, so use Arabic letters that are: "سلام دنیا" (CP1256 bytes).
    let mut ku_bytes = b"1\r\n00:00:00,500 --> 00:00:05,500\r\n".to_vec();
    ku_bytes.extend_from_slice(&[0xD3, 0xE1, 0xC7, 0xE3, 0x20, 0xCF, 0xE4, 0xED, 0xC7, 0x0D, 0x0A, 0x0D, 0x0A]);
    let _ = std::fs::write(&ku, ku_bytes);
    let sup = dir.join("subs.sup");
    write_pgs(&sup);
    let subbed = dir.join("sample_subs.mkv");
    let _ = engine::hidden_command(&ff)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(&a)
        .arg("-i")
        .arg(&srt)
        .arg("-i")
        .arg(&sup)
        .args(["-map", "0", "-map", "1", "-map", "2", "-c", "copy", "-c:s:0", "srt"])
        .arg(&subbed)
        .status();
    let info_s = engine::probe(&subbed);
    println!("subtitle tracks: {:?}", info_s.subs.iter().map(|s| (s.codec.clone(), s.text_based)).collect::<Vec<_>>());
    let sub_in = |choice: SubChoice| JobInput { subtitle: choice, ..input(&subbed, &info_s, Edits::default()) };
    add("sub_text", find("MP4 – works everywhere (H.264)"), vec![sub_in(SubChoice::Burn(0))], AccelChoice::Auto, 0.0);
    add("sub_picture", find("MP4 – works everywhere (H.264)"), vec![sub_in(SubChoice::Burn(1))], AccelChoice::Auto, 0.0);
    add("sub_picture_cpu", find("MKV – H.265 / HEVC"), vec![sub_in(SubChoice::Burn(1))], AccelChoice::CpuOnly, 0.0);
    add("sub_external_cp1256", find("MP4 – works everywhere (H.264)"), vec![sub_in(SubChoice::External(ku.clone()))], AccelChoice::Auto, 0.0);
    let trimmed = Edits { trim_start: 1.0, ..Default::default() };
    add("sub_text_trim", find("MP4 – H.264"), vec![JobInput { edits: trimmed, ..sub_in(SubChoice::Burn(0)) }], AccelChoice::CpuOnly, 0.0);

    // ---- size estimate vs. real size on a busy 1080p clip
    let busy = dir.join("sample_busy.mp4");
    let _ = engine::hidden_command(&ff)
        .args(["-hide_banner", "-loglevel", "error", "-y", "-f", "lavfi", "-i", "testsrc2=s=1920x1080:r=25:d=20", "-f", "lavfi", "-i", "sine=frequency=440:sample_rate=44100:d=20"])
        .args(["-vf", "noise=alls=25:allf=t+u", "-c:v", "libx264", "-b:v", "2444k", "-maxrate", "3000k", "-bufsize", "5000k", "-pix_fmt", "yuv420p", "-c:a", "aac", "-b:a", "128k", "-shortest"])
        .arg(&busy)
        .status();
    let info_busy = engine::probe(&busy);
    let mut size_checks: Vec<(String, u64)> = Vec::new();
    for (label, prof, accel) in [
        ("size_gpu_h264", "MP4 – works everywhere (H.264)", AccelChoice::Auto),
        ("size_cpu_h264", "MP4 – works everywhere (H.264)", AccelChoice::CpuOnly),
        ("size_gpu_hevc", "MP4 – H.265 / HEVC", AccelChoice::Auto),
        ("size_cpu_720p", "MP4 – tiny file for sharing (720p)", AccelChoice::CpuOnly),
    ] {
        let p = find(prof);
        let est = engine::estimate_output(&p, &info_busy, &Edits::default(), 0).unwrap_or(0);
        size_checks.push((label.to_string(), est));
        add(label, p, vec![input(&busy, &info_busy, Edits::default())], accel, 0.0);
    }

    let mut pass = 0;
    let total = cases.len();
    for (label, spec) in cases {
        let out = spec.output.clone();
        let split = spec.split_minutes > 0.0;
        let job = engine::start_job(spec, ctx.clone());
        loop {
            std::thread::sleep(std::time::Duration::from_millis(100));
            if job.state.lock().unwrap().finished {
                break;
            }
        }
        let st = job.state.lock().unwrap().clone();
        let produced = if split {
            std::fs::read_dir(&dir).map(|r| r.filter_map(|e| e.ok()).filter(|e| e.file_name().to_string_lossy().starts_with("out_split_part")).count()).unwrap_or(0)
        } else if out.exists() {
            1
        } else {
            0
        };
        let mut ok = st.ok && produced > 0;
        if let Some((_, est)) = size_checks.iter().find(|(l, _)| *l == label) {
            let real = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
            let ratio = real as f64 / (*est).max(1) as f64;
            println!("     size: estimated {} · real {} · real/estimate = {:.2}", engine::fmt_bytes(*est), engine::fmt_bytes(real), ratio);
            ok &= (0.8..=1.2).contains(&ratio);
        }
        if label.starts_with("sub_") && out.exists() {
            if let Some(img) = engine::grab_frame(&out, 2.0, 640, &[]) {
                let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
                if let Some(buf) = image::RgbaImage::from_raw(img.size[0] as u32, img.size[1] as u32, bytes) {
                    let _ = buf.save(dir.join(format!("frame_{}.png", label)));
                }
            }
        }
        if split {
            println!("split cmd: {}", st.command);
        }
        if ok {
            pass += 1;
        }
        let dur = if split || !out.exists() { 0.0 } else { engine::probe(&out).duration };
        println!(
            "{} {:<14} engine={:<22} fallback={} files={} dur={:.2}s size={} time={:.1}s {}",
            if ok { "PASS" } else { "FAIL" },
            label,
            st.engine,
            st.fell_back,
            produced,
            dur,
            engine::fmt_bytes(st.out_bytes),
            st.elapsed,
            if ok { String::new() } else { format!("\n     error: {}\n     cmd: {}", st.error, st.command) }
        );
    }
    println!("{}/{} passed", pass, total);
}
