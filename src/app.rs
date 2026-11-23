use crate::bidi;
use crate::engine::{self, Edits, JobHandle, JobInput, JobSpec, JobState, MediaInfo, SubChoice};
use crate::gpu::{self, AccelChoice, GpuReport, Vendor};
use crate::profiles::{self, ACodec, Fit, Profile, Quality, VCodec, CATEGORIES};
use crate::settings::{ExistsPolicy, Settings};
use eframe::egui::{self, Color32, RichText};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const ACCENT: Color32 = Color32::from_rgb(47, 124, 246);
const GOOD: Color32 = Color32::from_rgb(52, 199, 89);
const WARN: Color32 = Color32::from_rgb(255, 159, 10);
const BAD: Color32 = Color32::from_rgb(255, 69, 58);

const MEDIA_EXT: &[&str] = &[
    "mp4", "m4v", "mov", "mkv", "avi", "wmv", "asf", "flv", "f4v", "webm", "mpg", "mpeg", "m2v", "vob", "ts", "m2ts", "mts", "tod", "mod", "3gp", "3g2", "ogv", "ogg", "dv", "mxf", "rm", "rmvb",
    "divx", "xvid", "evo", "y4m", "gif", "mp3", "m4a", "aac", "flac", "wav", "aiff", "aif", "wma", "opus", "ac3", "eac3", "dts", "mka", "amr", "ape", "alac", "m4b", "mp2", "au", "caf",
];

const SUBTITLE_EXT: &[&str] = &["srt", "ass", "ssa", "vtt"];
const TITLE: &str = "ReelForge – Video & Audio Converter";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Status {
    Ready,
    Queued,
    Running,
    Done,
    Failed,
    Skipped,
    Cancelled,
}

struct Task {
    id: u64,
    path: PathBuf,
    name: String,
    info: Option<MediaInfo>,
    checked: bool,
    edits: Edits,
    audio_track: usize,
    subtitle: SubChoice,
    forced_only: bool,
    sub_size: u32,
    split_minutes: f64,
    status: Status,
    job: Option<JobHandle>,
    result: Option<JobState>,
    output: Option<PathBuf>,
}

impl Task {
    fn duration(&self) -> f64 {
        self.info.as_ref().map(|i| i.duration).unwrap_or(0.0)
    }
}

#[derive(PartialEq, Clone, Copy)]
enum EditTab {
    Trim,
    Crop,
    Effects,
    Watermark,
    Audio,
    Subtitle,
}

#[derive(PartialEq, Clone, Copy)]
enum OptTab {
    General,
    Conversion,
    Acceleration,
    Snapshot,
}

pub struct App {
    settings: Settings,
    library: Vec<Profile>,
    current: Profile,
    tasks: Vec<Task>,
    next_id: u64,
    selected: Option<u64>,
    gpu: Arc<Mutex<GpuReport>>,
    probes: engine::ProbePool,
    frame_inbox: Arc<Mutex<Vec<(u64, egui::ColorImage)>>>,
    preview_tex: Option<(u64, egui::TextureHandle)>,
    preview_time: f64,
    preview_wanted: Option<(u64, f64, Vec<String>)>,
    preview_busy: bool,
    preview_key: Option<(u64, i64, Vec<String>)>,
    show_picker: bool,
    picker_search: String,
    picker_cat: String,
    show_customize: bool,
    show_options: bool,
    opt_tab: OptTab,
    edit_task: Option<u64>,
    edit_tab: EditTab,
    log_task: Option<u64>,
    show_about: bool,
    merge: bool,
    converting: bool,
    merge_job: Option<(JobHandle, PathBuf, Vec<u64>)>,
    shutdown_after: bool,
    /// Windows was told to shut down in a minute (can still be cancelled).
    shutdown_pending: bool,
    /// Files of the conversion that is running (or ran last).
    batch: Vec<u64>,
    /// Files written by this conversion, so two jobs never share a name.
    batch_outputs: Vec<PathBuf>,
    batch_merged: bool,
    /// Stop was pressed: no "finished" actions (open folder, shut down).
    stopped: bool,
    /// Overall progress of the running conversion, 0–1.
    overall: f32,
    title_pct: Option<u32>,
    message: Option<(String, Color32)>,
    started_at: Option<std::time::Instant>,
    ctx: egui::Context,
    screenshot: Option<PathBuf>,
    frames: u32,
    wait_frames: u32,
    /// Settings are written to disk (not for documentation pictures or tests).
    persist: bool,
    /// False in tests: no FFmpeg is started for previews.
    live: bool,
    /// Test aid (`REELFORGE_AUTOCONVERT` with `--screenshot`): 1 = convert the
    /// files once they are read, 2 = converting, 0 = off / finished.
    auto_convert: u8,
    /// Bring the selected row into view (after ↑ / ↓).
    scroll_to_selected: bool,
}

fn open_path(p: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(p).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(p).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(p).spawn();
    }
}

fn reveal(p: &Path) {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("explorer").arg(format!("/select,{}", p.display())).spawn();
    }
    #[cfg(not(windows))]
    {
        if let Some(d) = p.parent() {
            open_path(d);
        }
    }
}

fn is_media(p: &Path) -> bool {
    p.extension().map(|e| MEDIA_EXT.contains(&e.to_string_lossy().to_lowercase().as_str())).unwrap_or(false)
}

fn is_subtitle(p: &Path) -> bool {
    p.extension().map(|e| SUBTITLE_EXT.contains(&e.to_string_lossy().to_lowercase().as_str())).unwrap_or(false)
}

fn file_name(p: &Path) -> String {
    p.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

/// Windows file names ignore upper and lower case, so "CLIP.MP4" and
/// "clip.mp4" are one file.
fn same_path(a: &Path, b: &Path) -> bool {
    let norm = |p: &Path| p.to_string_lossy().replace('/', "\\").to_lowercase();
    norm(a) == norm(b)
}

/// Where a converted file goes. Never the source file itself and never a name
/// another job of this conversion already uses (`taken`); otherwise an
/// existing file is handled as the options say. `None` = skip this file.
fn choose_output(dir: &Path, stem: &str, ext: &str, source: &Path, policy: ExistsPolicy, exists: &dyn Fn(&Path) -> bool, taken: &dyn Fn(&Path) -> bool) -> Option<PathBuf> {
    let mut out = dir.join(format!("{}.{}", stem, ext));
    let mut n = 1;
    loop {
        if !same_path(&out, source) && !taken(&out) {
            if !exists(&out) {
                return Some(out);
            }
            match policy {
                ExistsPolicy::Overwrite => return Some(out),
                ExistsPolicy::Skip => return None,
                ExistsPolicy::Rename => {}
            }
        }
        out = dir.join(format!("{} ({}).{}", stem, n, ext));
        n += 1;
    }
}

/// Which files Convert works on: `(id, already converted)` of every ticked,
/// readable file. Files that are done are left alone while others still wait,
/// so pressing Convert after a stop or a failure carries on instead of
/// starting over. When everything is done (or for a merge) all are taken.
fn pick_targets(ticked: &[(u64, bool)], merge: bool) -> Vec<u64> {
    let pending: Vec<u64> = ticked.iter().filter(|(_, done)| !done).map(|(id, _)| *id).collect();
    if pending.is_empty() || (merge && ticked.len() > 1) {
        ticked.iter().map(|(id, _)| *id).collect()
    } else {
        pending
    }
}

/// The video a dropped subtitle file belongs to: the one with the same name
/// ("film.srt", "film.en.srt" → "film.mkv"), else the selected one, else the
/// only one. `videos` = `(id, lower-case name without extension)`.
fn subtitle_target(videos: &[(u64, String)], sub_stem: &str, selected: Option<u64>) -> Option<u64> {
    let sub = sub_stem.to_lowercase();
    if let Some((id, _)) = videos.iter().find(|(_, v)| *v == sub) {
        return Some(*id);
    }
    let by_prefix = videos.iter().filter(|(_, v)| !v.is_empty() && sub.starts_with(&format!("{}.", v))).max_by_key(|(_, v)| v.len());
    if let Some((id, _)) = by_prefix {
        return Some(*id);
    }
    if let Some(id) = selected.filter(|id| videos.iter().any(|(v, _)| v == id)) {
        return Some(id);
    }
    match videos {
        [(id, _)] => Some(*id),
        _ => None,
    }
}

/// Overall progress of a conversion: `(length in seconds, progress 0–1)` per
/// file, so a long film counts for more than a short clip.
fn overall_progress(parts: &[(f64, f32)]) -> f32 {
    let total: f64 = parts.iter().map(|(d, _)| d.max(1.0)).sum();
    if total <= 0.0 {
        return 0.0;
    }
    let done: f64 = parts.iter().map(|(d, p)| d.max(1.0) * (*p as f64).clamp(0.0, 1.0)).sum();
    (done / total) as f32
}

/// Seconds left, judged from the time taken so far. `None` while it is too
/// early to say.
fn time_left(elapsed: f64, progress: f32) -> Option<f64> {
    if progress < 0.03 || progress >= 1.0 || elapsed < 2.0 {
        return None;
    }
    Some(elapsed * (1.0 - progress as f64) / progress as f64)
}

#[derive(Default, Debug, Clone, PartialEq)]
struct BatchStats {
    done: usize,
    failed: usize,
    cancelled: usize,
    skipped: usize,
    gpu: usize,
    secs: f64,
    /// Sizes of the originals and the new files, for files converted whole.
    in_bytes: u64,
    out_bytes: u64,
    stopped: bool,
}

/// The line shown when a conversion ends.
fn summary_text(s: &BatchStats) -> String {
    let mut msg = if s.stopped { format!("Stopped: {} converted", s.done) } else { format!("Finished: {} converted in {}", s.done, engine::fmt_time(s.secs)) };
    if s.done > 0 && s.in_bytes > 0 && s.out_bytes > 0 {
        let change = (1.0 - s.out_bytes as f64 / s.in_bytes as f64) * 100.0;
        let word = if change.abs() < 0.5 {
            "same size".to_string()
        } else if change > 0.0 {
            format!("{:.0}% smaller", change)
        } else {
            format!("{:.0}% larger", -change)
        };
        msg.push_str(&format!(" · {} → {} ({})", engine::fmt_bytes(s.in_bytes), engine::fmt_bytes(s.out_bytes), word));
    }
    if s.gpu > 0 {
        msg.push_str(&format!(" · {} with the graphics card", s.gpu));
    }
    if s.skipped > 0 {
        msg.push_str(&format!(" · {} skipped (file already there)", s.skipped));
    }
    if s.cancelled > 0 && !s.stopped {
        msg.push_str(&format!(" · {} cancelled", s.cancelled));
    }
    if s.failed > 0 {
        msg.push_str(&format!(" · {} failed (click the status to see why)", s.failed));
    }
    msg
}

/// A label in a colour; right-to-left text (Kurdish, Arabic…) is drawn joined
/// and in reading order.
fn colored(ui: &mut egui::Ui, text: &str, color: Color32) -> egui::Response {
    if bidi::needs_native(text) {
        let size = egui::TextStyle::Body.resolve(ui.style()).size;
        bidi::label(ui, text, size, color)
    } else {
        ui.label(RichText::new(text).color(color))
    }
}

fn status_chip(ui: &mut egui::Ui, text: String, col: Color32) -> egui::Response {
    egui::Frame::new().fill(col.gamma_multiply(0.18)).corner_radius(12).inner_margin(egui::Margin::symmetric(10, 4)).show(ui, |ui| ui.label(RichText::new(text).color(col).strong())).response
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>, screenshot: Option<PathBuf>) -> Self {
        Self::build(&cc.egui_ctx, screenshot, true)
    }

    /// `live` false (tests): no graphics-card check, no FFmpeg, nothing saved.
    fn build(ctx: &egui::Context, screenshot: Option<PathBuf>, live: bool) -> Self {
        // Pictures for the documentation use neutral settings and never save them.
        let persist = live && screenshot.is_none();
        let settings = if persist { Settings::load() } else { Settings::for_screenshot() };
        let mut library = profiles::library();
        library.extend(settings.custom_profiles.iter().cloned());
        let current = library.iter().find(|p| p.id == settings.profile_id).cloned().unwrap_or_else(|| library[0].clone());
        let gpu = Arc::new(Mutex::new(GpuReport { done: !live, ..Default::default() }));
        if live {
            gpu::detect_async(gpu.clone(), ctx.clone());
        }
        crate::text::install_fonts(ctx);
        let app = App {
            persist,
            live,
            picker_cat: "Recommended".into(),
            settings,
            library,
            current,
            tasks: vec![],
            next_id: 1,
            selected: None,
            gpu,
            probes: engine::ProbePool::default(),
            frame_inbox: Arc::new(Mutex::new(vec![])),
            preview_tex: None,
            preview_time: 0.0,
            preview_wanted: None,
            preview_busy: false,
            preview_key: None,
            show_picker: false,
            picker_search: String::new(),
            show_customize: false,
            show_options: false,
            opt_tab: OptTab::General,
            edit_task: None,
            edit_tab: EditTab::Trim,
            log_task: None,
            show_about: false,
            merge: false,
            converting: false,
            merge_job: None,
            shutdown_after: false,
            shutdown_pending: false,
            batch: vec![],
            batch_outputs: vec![],
            batch_merged: false,
            stopped: false,
            overall: 0.0,
            title_pct: None,
            message: None,
            started_at: None,
            ctx: ctx.clone(),
            screenshot,
            frames: 0,
            wait_frames: 0,
            auto_convert: 0,
            scroll_to_selected: false,
        };
        app.apply_theme(ctx);
        let mut app = app;
        // Test aid: open the format picker straight away.
        if app.screenshot.is_some() {
            // Test aids for documentation pictures: convert into a given folder.
            if let Some(dir) = std::env::var_os("REELFORGE_OUTPUT_DIR") {
                app.settings.output_dir = PathBuf::from(dir);
                app.auto_convert = std::env::var_os("REELFORGE_AUTOCONVERT").is_some() as u8;
            }
        }
        if std::env::var_os("REELFORGE_SHOW_CUSTOMIZE").is_some() {
            app.show_customize = true;
        }
        if std::env::var_os("REELFORGE_SHOW_PICKER").is_some() {
            app.show_picker = true;
            app.picker_cat = std::env::var("REELFORGE_SHOW_PICKER").unwrap_or_default();
            if !CATEGORIES.iter().any(|(c, _)| *c == app.picker_cat) {
                app.picker_cat = "Recommended".into();
            }
        }
        app
    }

    fn apply_theme(&self, ctx: &egui::Context) {
        let mut v = if self.settings.dark { egui::Visuals::dark() } else { egui::Visuals::light() };
        v.selection.bg_fill = ACCENT;
        v.hyperlink_color = ACCENT;
        if self.settings.dark {
            v.panel_fill = Color32::from_rgb(28, 31, 38);
            v.window_fill = Color32::from_rgb(34, 38, 46);
            v.extreme_bg_color = Color32::from_rgb(20, 22, 27);
            v.faint_bg_color = Color32::from_rgb(36, 40, 49);
            v.widgets.inactive.weak_bg_fill = Color32::from_rgb(46, 51, 62);
            v.widgets.inactive.bg_fill = Color32::from_rgb(46, 51, 62);
            v.widgets.hovered.weak_bg_fill = Color32::from_rgb(60, 67, 82);
        } else {
            v.panel_fill = Color32::from_rgb(244, 246, 250);
            v.faint_bg_color = Color32::from_rgb(232, 236, 243);
        }
        v.window_corner_radius = egui::CornerRadius::same(10);
        v.widgets.inactive.corner_radius = egui::CornerRadius::same(6);
        v.widgets.hovered.corner_radius = egui::CornerRadius::same(6);
        v.widgets.active.corner_radius = egui::CornerRadius::same(6);
        ctx.set_visuals(v);
        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(10.0, 5.0);
        ctx.set_style(style);
    }

    // ------------------------------------------------------------ tasks

    fn add_paths(&mut self, paths: Vec<PathBuf>) {
        let mut files = Vec::new();
        let mut subtitles = Vec::new();
        for p in paths {
            if p.is_dir() {
                collect_dir(&p, &mut files, 0);
            } else if p.is_file() {
                if is_subtitle(&p) {
                    subtitles.push(p);
                } else {
                    files.push(p);
                }
            }
        }
        let mut added = 0;
        for f in files {
            if self.tasks.iter().any(|t| t.path == f) {
                continue;
            }
            let id = self.push_task(f.clone());
            self.probes.submit(id, f, &self.ctx);
            added += 1;
        }
        if added > 0 {
            self.message = Some((format!("Added {} file{}", added, if added == 1 { "" } else { "s" }), ACCENT));
        }
        for sub in subtitles {
            self.attach_subtitle(sub);
        }
    }

    /// Puts a file on the list (its details are read afterwards).
    fn push_task(&mut self, path: PathBuf) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        self.tasks.push(Task {
            id,
            name: file_name(&path),
            path,
            info: None,
            checked: true,
            edits: Edits::default(),
            audio_track: 0,
            subtitle: SubChoice::None,
            forced_only: false,
            sub_size: 0,
            split_minutes: 0.0,
            status: Status::Ready,
            job: None,
            result: None,
            output: None,
        });
        if self.selected.is_none() {
            self.selected = Some(id);
        }
        id
    }

    /// A subtitle file was dropped or picked together with videos: use it for
    /// the video it belongs to.
    fn attach_subtitle(&mut self, file: PathBuf) {
        let stem_of = |p: &Path| p.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
        let videos: Vec<(u64, String)> =
            self.tasks.iter().filter(|t| t.job.is_none() && t.status != Status::Running && t.info.as_ref().map(|i| i.ok && i.has_video).unwrap_or(true)).map(|t| (t.id, stem_of(&t.path))).collect();
        let Some(id) = subtitle_target(&videos, &stem_of(&file), self.selected) else {
            self.message = Some(("Select the video first, then drop its subtitle file".into(), WARN));
            return;
        };
        let mut name = String::new();
        if let Some(t) = self.task_mut(id) {
            t.subtitle = SubChoice::External(file);
            name = t.name.clone();
        }
        if self.selected == Some(id) {
            self.preview_key = None;
        }
        self.message = Some((format!("Subtitles will be burned into {}", name), GOOD));
    }

    pub fn add_files(&mut self, paths: Vec<PathBuf>) {
        self.add_paths(paths);
    }

    fn task(&self, id: u64) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }
    fn task_mut(&mut self, id: u64) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    fn poll(&mut self) {
        let probes: Vec<_> = std::mem::take(&mut *self.probes.inbox.lock().unwrap());
        for (id, info) in probes {
            let sel = self.selected == Some(id);
            if let Some(t) = self.task_mut(id) {
                if !info.ok {
                    t.status = Status::Failed;
                    t.checked = false;
                    t.result = Some(JobState { error: format!("Can't read this file: {}", info.error), finished: true, ..Default::default() });
                }
                t.info = Some(info);
            }
            if sel {
                self.preview_time = self.task(id).map(|t| (t.duration() * 0.1).min(30.0)).unwrap_or(0.0);
                self.preview_key = None;
                // Test aid: open the Subtitle tab with a subtitle chosen ("track" or a file path).
                if let Ok(v) = std::env::var("REELFORGE_EDIT_SUB") {
                    let choice = if let Some(n) = v.strip_prefix("track") { SubChoice::Burn(n.parse().unwrap_or(0)) } else { SubChoice::External(PathBuf::from(v)) };
                    let path = self.task(id).map(|t| t.path.clone()).unwrap_or_default();
                    if let Some(t0) = engine::first_subtitle_time(&path, &choice) {
                        self.preview_time = t0 + 0.4;
                    }
                    if let Some(t) = self.task_mut(id) {
                        t.subtitle = choice;
                    }
                    self.edit_task = Some(id);
                    self.edit_tab = EditTab::Subtitle;
                }
            }
        }
        let frames: Vec<_> = std::mem::take(&mut *self.frame_inbox.lock().unwrap());
        for (id, img) in frames {
            let tex = self.ctx.load_texture(format!("preview-{}", id), img, egui::TextureOptions::LINEAR);
            self.preview_tex = Some((id, tex));
            self.preview_busy = false;
        }
        if !self.preview_busy {
            if let Some((id, t, filters)) = self.preview_wanted.take() {
                let found = self.task(id).filter(|t| t.info.as_ref().map(|i| i.has_video).unwrap_or(false)).map(|t| t.path.clone());
                if let Some(path) = found {
                    {
                        self.preview_busy = true;
                        let inbox = self.frame_inbox.clone();
                        let ctx = self.ctx.clone();
                        std::thread::spawn(move || {
                            let img = engine::grab_frame(&path, t, 640, &filters).unwrap_or_else(|| egui::ColorImage::new([16, 9], Color32::BLACK));
                            inbox.lock().unwrap().push((id, img));
                            ctx.request_repaint();
                        });
                    }
                }
            }
        }
        for t in self.tasks.iter_mut() {
            if let Some(job) = &t.job {
                let st = job.state.lock().unwrap().clone();
                if st.finished {
                    t.status = if st.ok {
                        Status::Done
                    } else if st.cancelled {
                        Status::Cancelled
                    } else {
                        Status::Failed
                    };
                    t.job = None;
                }
                t.result = Some(st);
            }
        }
        if let Some((job, out, ids)) = &self.merge_job {
            let st = job.state.lock().unwrap().clone();
            let status = if st.finished {
                Some(if st.ok {
                    Status::Done
                } else if st.cancelled {
                    Status::Cancelled
                } else {
                    Status::Failed
                })
            } else {
                None
            };
            let out = out.clone();
            let ids = ids.clone();
            for id in &ids {
                if let Some(t) = self.task_mut(*id) {
                    t.result = Some(st.clone());
                    t.output = Some(out.clone());
                    t.status = status.unwrap_or(Status::Running);
                }
            }
            if status.is_some() {
                self.merge_job = None;
            }
        }
        if self.converting {
            self.schedule();
        }
    }

    fn output_for(&self, t: &Task, p: &Profile) -> Option<PathBuf> {
        let dir = if self.settings.same_folder_as_source { t.path.parent().map(|d| d.to_path_buf()).unwrap_or_else(|| self.settings.output_dir.clone()) } else { self.settings.output_dir.clone() };
        let _ = std::fs::create_dir_all(&dir);
        let stem = t.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "output".into());
        let taken = |o: &Path| self.batch_outputs.iter().any(|b| same_path(b, o));
        choose_output(&dir, &stem, &p.ext, &t.path, self.settings.exists, &|o| o.exists(), &taken)
    }

    /// The files the Convert button works on.
    fn convert_targets(&self) -> Vec<u64> {
        let ticked: Vec<(u64, bool)> = self
            .tasks
            .iter()
            .filter(|t| t.checked && t.info.as_ref().map(|i| i.ok).unwrap_or(false) && t.status != Status::Running && t.job.is_none())
            .map(|t| (t.id, t.status == Status::Done))
            .collect();
        pick_targets(&ticked, self.merge)
    }

    fn job_input(t: &Task) -> JobInput {
        JobInput {
            path: t.path.clone(),
            info: t.info.clone().unwrap_or_default(),
            edits: t.edits.clone(),
            audio_track: t.audio_track,
            subtitle: t.subtitle.clone(),
            forced_only: t.forced_only,
            sub_size: t.sub_size,
        }
    }

    fn hw_for(&self, p: &Profile) -> (Option<gpu::HwEncoder>, Option<Vendor>) {
        let rep = self.gpu.lock().unwrap();
        let enc = if self.settings.gpu_encode { rep.pick(p.vcodec, self.settings.accel).cloned() } else { None };
        let dec = if self.settings.gpu_decode && p.vcodec != VCodec::None && p.vcodec != VCodec::Copy { rep.decode_vendor(self.settings.accel) } else { None };
        (enc, dec)
    }

    fn start_conversion(&mut self) {
        let ready = self.convert_targets();
        if ready.is_empty() {
            self.message = Some(("Add some files and tick them first".into(), WARN));
            return;
        }
        if engine::ffmpeg_path().is_none() {
            self.message = Some(("FFmpeg was not found: put ffmpeg.exe and ffprobe.exe next to this program".into(), BAD));
            return;
        }
        if !self.settings.same_folder_as_source {
            if let Err(e) = std::fs::create_dir_all(&self.settings.output_dir) {
                self.message = Some((format!("Can't use the output folder: {}", e), BAD));
                return;
            }
        }
        self.batch = ready.clone();
        self.batch_outputs.clear();
        self.batch_merged = false;
        self.stopped = false;
        self.overall = 0.0;
        self.settings.push_recent(self.current.id);
        self.settings.profile_id = self.current.id;
        self.save_settings();
        self.started_at = Some(std::time::Instant::now());
        let p = self.current.clone();
        if self.merge && ready.len() > 1 {
            let first = self.task(ready[0]).unwrap();
            let stem = format!("{} (merged)", first.path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default());
            let dir = match first.path.parent() {
                Some(d) if self.settings.same_folder_as_source => d.to_path_buf(),
                _ => self.settings.output_dir.clone(),
            };
            // A merged file never replaces anything: it always gets a free name.
            let out = choose_output(&dir, &stem, &p.ext, &first.path, ExistsPolicy::Rename, &|o| o.exists(), &|_| false).unwrap_or_else(|| dir.join(format!("{}.{}", stem, p.ext)));
            self.batch_merged = true;
            self.batch_outputs.push(out.clone());
            let inputs: Vec<JobInput> = ready.iter().filter_map(|id| self.task(*id)).map(Self::job_input).collect();
            let (enc, dec) = self.hw_for(&p);
            let spec = JobSpec { inputs, profile: p, output: out.clone(), hw_encoder: enc, hw_decode: dec, threads: self.settings.threads, split_minutes: 0.0 };
            let job = engine::start_job(spec, self.ctx.clone());
            for id in &ready {
                if let Some(t) = self.task_mut(*id) {
                    t.status = Status::Running;
                    t.result = None;
                }
            }
            self.merge_job = Some((job, out, ready));
        } else {
            for id in ready {
                if let Some(t) = self.task_mut(id) {
                    t.status = Status::Queued;
                    t.result = None;
                }
            }
        }
        self.converting = true;
        self.message = None;
        self.schedule();
    }

    fn schedule(&mut self) {
        let running = self.tasks.iter().filter(|t| t.job.is_some()).count() + self.merge_job.is_some() as usize;
        let mut free = (self.settings.max_parallel.max(1) as usize).saturating_sub(running);
        let p = self.current.clone();
        let queued: Vec<u64> = self.tasks.iter().filter(|t| t.status == Status::Queued).map(|t| t.id).collect();
        for id in queued {
            if free == 0 {
                break;
            }
            let t = self.task(id).unwrap();
            let Some(out) = self.output_for(t, &p) else {
                if let Some(t) = self.task_mut(id) {
                    t.status = Status::Skipped;
                }
                continue;
            };
            let (enc, dec) = self.hw_for(&p);
            let spec =
                JobSpec { inputs: vec![Self::job_input(t)], profile: p.clone(), output: out.clone(), hw_encoder: enc, hw_decode: dec, threads: self.settings.threads, split_minutes: t.split_minutes };
            let job = engine::start_job(spec, self.ctx.clone());
            self.batch_outputs.push(out.clone());
            if let Some(t) = self.task_mut(id) {
                t.job = Some(job);
                t.status = Status::Running;
                t.output = Some(out);
            }
            free -= 1;
        }
        let busy = self.tasks.iter().any(|t| matches!(t.status, Status::Queued | Status::Running)) || self.merge_job.is_some();
        if !busy && self.converting {
            self.converting = false;
            self.finish_summary();
        }
    }

    fn batch_stats(&self) -> BatchStats {
        let batch: Vec<&Task> = self.tasks.iter().filter(|t| self.batch.contains(&t.id)).collect();
        let count = |st: Status| batch.iter().filter(|t| t.status == st).count();
        let mut stats = BatchStats {
            done: count(Status::Done),
            failed: count(Status::Failed),
            cancelled: count(Status::Cancelled),
            skipped: count(Status::Skipped),
            gpu: batch.iter().filter(|t| t.status == Status::Done && t.result.as_ref().map(|r| r.engine.starts_with("GPU") && !r.fell_back).unwrap_or(false)).count(),
            secs: self.started_at.map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0),
            stopped: self.stopped,
            ..Default::default()
        };
        // Compare sizes only for files converted whole (not trimmed or split).
        let whole = |t: &&&Task| t.status == Status::Done && t.edits.trim_start <= 0.0 && t.edits.trim_end <= 0.0 && t.split_minutes <= 0.0;
        let out_size = |t: &Task| t.result.as_ref().map(|r| r.out_bytes).unwrap_or(0);
        let in_size = |t: &Task| t.info.as_ref().map(|i| i.size).unwrap_or(0);
        if self.batch_merged {
            if !batch.is_empty() && batch.iter().all(|t| whole(&t)) {
                stats.in_bytes = batch.iter().map(|t| in_size(t)).sum();
                stats.out_bytes = out_size(batch[0]);
            }
            // One merged file, however many parts went into it.
            stats.done = stats.done.min(1);
            stats.gpu = stats.gpu.min(1);
            stats.failed = stats.failed.min(1);
            stats.cancelled = stats.cancelled.min(1);
        } else {
            for t in batch.iter().filter(whole).filter(|t| out_size(t) > 0 && in_size(t) > 0) {
                stats.in_bytes += in_size(t);
                stats.out_bytes += out_size(t);
            }
        }
        stats
    }

    fn finish_summary(&mut self) {
        let stats = self.batch_stats();
        let trouble = stats.failed > 0 || stats.stopped;
        self.message = Some((summary_text(&stats), if trouble { WARN } else { GOOD }));
        // Stop means stop: nothing is opened and the computer stays on.
        if stats.done == 0 || stats.stopped {
            return;
        }
        if self.settings.open_folder_after {
            // The folder the files really went to ("Next to the original" too).
            let last = self.tasks.iter().rev().find(|t| self.batch.contains(&t.id) && t.status == Status::Done).and_then(|t| t.output.as_ref()).and_then(|o| o.parent()).map(|d| d.to_path_buf());
            open_path(&last.unwrap_or_else(|| self.settings.output_dir.clone()));
        }
        if self.shutdown_after {
            #[cfg(windows)]
            {
                let _ = engine::hidden_command("shutdown").args(["/s", "/t", "60"]).spawn();
                self.shutdown_pending = true;
                self.message = Some(("Conversion done – the computer will shut down in 60 seconds".into(), WARN));
            }
        }
    }

    fn cancel_shutdown(&mut self) {
        #[cfg(windows)]
        {
            let _ = engine::hidden_command("shutdown").arg("/a").spawn();
        }
        self.shutdown_pending = false;
        self.shutdown_after = false;
        self.message = Some(("Shutdown cancelled".into(), GOOD));
    }

    fn stop_all(&mut self) {
        for t in self.tasks.iter_mut() {
            if let Some(j) = &t.job {
                j.cancel();
            }
            if t.status == Status::Queued {
                t.status = Status::Ready;
            }
        }
        if let Some((j, _, _)) = &self.merge_job {
            j.cancel();
        }
        self.stopped = true;
        self.message = Some(("Conversion stopped".into(), WARN));
    }

    fn save_settings(&self) {
        if self.persist {
            self.settings.save();
        }
    }

    fn request_preview(&mut self) {
        if !self.live {
            return;
        }
        if let Some(id) = self.selected {
            let at = self.preview_time;
            let filters = self.task(id).map(|t| {
                let mut f = Vec::new();
                let subs = t.info.as_ref().map(|i| i.subs.clone()).unwrap_or_default();
                // Picture subtitles go first (drawn on the full-size frame).
                if let SubChoice::Burn(k) = t.subtitle {
                    if subs.get(k).map(|s| !s.text_based).unwrap_or(false) {
                        f.push(format!("@picsub:{}", k));
                    }
                }
                f.extend(t.edits.picture_filters());
                f.extend(engine::text_subtitle_filters(&t.path, &t.subtitle, &subs, at, t.sub_size));
                if let Some(w) = t.edits.watermark_filter() {
                    f.push(w);
                }
                f
            });
            if let Some(filters) = filters {
                let key = (id, (self.preview_time * 10.0) as i64, filters.clone());
                if self.preview_key.as_ref() != Some(&key) {
                    self.preview_key = Some(key);
                    self.preview_wanted = Some((id, self.preview_time, filters));
                }
            }
        }
    }

    fn set_profile(&mut self, p: Profile) {
        self.settings.profile_id = p.id;
        self.current = p;
        self.save_settings();
    }

    fn select(&mut self, id: u64) {
        if self.selected != Some(id) {
            self.selected = Some(id);
            self.preview_time = self.task(id).map(|t| (t.duration() * 0.1).min(30.0)).unwrap_or(0.0);
            self.preview_key = None;
        }
    }

    /// Takes a file off the list (not while it is being converted).
    fn remove_task(&mut self, id: u64) {
        if let Some(i) = self.tasks.iter().position(|t| t.id == id) {
            if self.tasks[i].job.is_none() && self.tasks[i].status != Status::Running {
                self.tasks.remove(i);
                if self.selected == Some(id) {
                    self.selected = None;
                    self.preview_tex = None;
                    if let Some(next) = self.tasks.get(i.min(self.tasks.len().saturating_sub(1))).map(|t| t.id) {
                        self.select(next);
                    }
                }
            }
        }
    }

    fn open_edit(&mut self, id: u64) {
        self.selected = Some(id);
        self.edit_task = Some(id);
        self.preview_time = self.task(id).map(|t| t.edits.trim_start).unwrap_or(0.0);
        self.preview_key = None;
    }

    /// Keyboard: Esc closes the open window; in the file list ↑ ↓ move the
    /// selection, Delete removes, Enter edits, Ctrl+O adds files, Ctrl+A ticks all.
    fn shortcuts(&mut self, ctx: &egui::Context) {
        let window_open = self.show_picker || self.show_customize || self.show_options || self.edit_task.is_some() || self.log_task.is_some() || self.show_about;
        if window_open {
            let popup = ctx.memory(|m| m.any_popup_open());
            if !popup && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                if self.show_about {
                    self.show_about = false;
                } else if self.log_task.is_some() {
                    self.log_task = None;
                } else if self.edit_task.is_some() {
                    self.edit_task = None;
                } else if self.show_options {
                    self.show_options = false;
                    self.save_settings();
                } else if self.show_customize {
                    self.show_customize = false;
                } else {
                    self.show_picker = false;
                }
            }
            return;
        }
        // Typing in a box or choosing from an open drop-down: the keys belong there.
        if ctx.wants_keyboard_input() || ctx.memory(|m| m.any_popup_open()) {
            return;
        }
        let (up, down, del, enter, open, all) = ctx.input_mut(|i| {
            (
                i.key_pressed(egui::Key::ArrowUp),
                i.key_pressed(egui::Key::ArrowDown),
                i.key_pressed(egui::Key::Delete),
                i.key_pressed(egui::Key::Enter),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::O),
                i.consume_key(egui::Modifiers::COMMAND, egui::Key::A),
            )
        });
        let idx = self.selected.and_then(|id| self.tasks.iter().position(|t| t.id == id));
        if up || down {
            let next = match idx {
                Some(i) if up => i.saturating_sub(1),
                Some(i) => (i + 1).min(self.tasks.len().saturating_sub(1)),
                None => 0,
            };
            if let Some(id) = self.tasks.get(next).map(|t| t.id) {
                self.select(id);
                self.scroll_to_selected = true;
            }
        }
        if let Some(id) = self.selected {
            if del {
                self.remove_task(id);
            } else if enter {
                self.open_edit(id);
            }
        }
        if all {
            for t in self.tasks.iter_mut() {
                t.checked = true;
            }
        }
        if open {
            self.pick_files();
        }
    }

    fn pick_files(&mut self) {
        let mut with_subs: Vec<&str> = MEDIA_EXT.to_vec();
        with_subs.extend_from_slice(SUBTITLE_EXT);
        if let Some(files) = rfd::FileDialog::new().add_filter("Media and subtitle files", &with_subs).add_filter("Media files", MEDIA_EXT).add_filter("All files", &["*"]).pick_files() {
            self.add_paths(files);
        }
    }

    fn pick_folder(&mut self) {
        if let Some(d) = rfd::FileDialog::new().pick_folder() {
            self.add_paths(vec![d]);
        }
    }

    // ------------------------------------------------------------ panels

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(RichText::new("⚡ ReelForge").size(20.0).strong().color(ACCENT));
            ui.add_space(14.0);
            let big = |t: &str| egui::Button::new(RichText::new(t).size(14.0)).min_size(egui::vec2(0.0, 32.0));
            if ui.add(big("➕  Add files")).on_hover_text("Load video or audio files (Ctrl+O)").clicked() {
                self.pick_files();
            }
            if ui.add(big("📁  Add folder")).on_hover_text("Add every video and audio file in a folder").clicked() {
                self.pick_folder();
            }
            if ui.add(big("💿  Add disc folder")).on_hover_text("Add an unprotected DVD (VIDEO_TS) or Blu-ray (BDMV) folder").clicked() {
                if let Some(d) = rfd::FileDialog::new().pick_folder() {
                    let files = disc_titles(&d);
                    if files.is_empty() {
                        self.message = Some(("No DVD or Blu-ray video found in that folder".into(), WARN));
                    } else {
                        self.add_paths(files);
                    }
                }
            }
            if ui.add_enabled(self.selected.is_some(), big("✂  Edit")).on_hover_text("Trim, crop, rotate, adjust colours, add text (Enter)").clicked() {
                if let Some(id) = self.selected {
                    self.open_edit(id);
                }
            }
            if ui.add(big("⚙  Options")).clicked() {
                self.show_options = true;
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("?").on_hover_text("About").clicked() {
                    self.show_about = true;
                }
                let rep = self.gpu.lock().unwrap().clone();
                let picked = rep.pick(VCodec::H264, self.settings.accel).cloned();
                let (txt, col) = if !rep.done {
                    ("⏳ Checking graphics cards…".to_string(), Color32::GRAY)
                } else if let (true, Some(e)) = (self.settings.gpu_encode, picked) {
                    (format!("⚡ GPU: {}", rep.adapter_for(e.vendor).map(|a| a.name.clone()).unwrap_or_else(|| e.vendor.short().into())), GOOD)
                } else if !self.settings.gpu_encode && !rep.encoders.is_empty() {
                    ("GPU off (Options)".into(), WARN)
                } else {
                    ("🖥 Processor only".into(), Color32::GRAY)
                };
                let r = status_chip(ui, txt, col);
                if r.interact(egui::Sense::click()).on_hover_text("Click to change acceleration settings").clicked() {
                    self.show_options = true;
                    self.opt_tab = OptTab::Acceleration;
                }
            });
        });
    }

    fn bottom_bar(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Format").strong().size(14.0));
            let icon = profiles::category_icon(&self.current.category);
            let btn = egui::Button::new(RichText::new(format!("{} {}   ⏷", icon, self.current.name)).size(14.0)).min_size(egui::vec2(380.0, 30.0));
            if ui.add(btn).on_hover_text("Choose from all conversion profiles").clicked() {
                self.show_picker = true;
                self.picker_cat = self.current.category.clone();
            }
            if ui.button("⚙ Customize").on_hover_text("Change codec, size, quality, sound…").clicked() {
                self.show_customize = true;
            }
            ui.label(RichText::new(self.current.summary()).weak());
        });
        let recent: Vec<Profile> = self.settings.recent.iter().filter(|id| **id != self.current.id).filter_map(|id| self.library.iter().find(|p| p.id == *id).cloned()).take(4).collect();
        if !recent.is_empty() {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Recent:").weak());
                for p in recent {
                    if ui.small_button(&p.name).clicked() {
                        self.set_profile(p);
                    }
                }
            });
        }
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label(RichText::new("Save to").strong().size(14.0));
            let mut s = self.settings.output_dir.to_string_lossy().to_string();
            let same = self.settings.same_folder_as_source;
            ui.add_enabled_ui(!same, |ui| {
                if bidi::text_edit(ui, &mut s, 300.0, "").changed() {
                    self.settings.output_dir = PathBuf::from(&s);
                }
                if ui.button("📂 Browse").on_hover_text("Set output folder").clicked() {
                    if let Some(d) = rfd::FileDialog::new().set_directory(&self.settings.output_dir).pick_folder() {
                        self.settings.output_dir = d;
                        self.save_settings();
                    }
                }
            });
            if ui.button("Open").on_hover_text("Open the output folder").clicked() {
                let _ = std::fs::create_dir_all(&self.settings.output_dir);
                open_path(&self.settings.output_dir);
            }
            if ui.checkbox(&mut self.settings.same_folder_as_source, "Next to the original").changed() {
                self.save_settings();
            }
            ui.checkbox(&mut self.merge, "Merge into one file").on_hover_text("Combine the ticked files into one video");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if self.converting {
                    let b = egui::Button::new(RichText::new("⏹  Stop").size(18.0).strong().color(Color32::WHITE)).fill(BAD).min_size(egui::vec2(170.0, 44.0));
                    if ui.add(b).clicked() {
                        self.stop_all();
                    }
                } else {
                    let n = self.convert_targets().len();
                    let ticked = self.tasks.iter().filter(|t| t.checked && t.info.as_ref().map(|i| i.ok).unwrap_or(false)).count();
                    let label = if n > 0 { format!("▶  Convert {}", n) } else { "▶  Convert".into() };
                    let hint = if n > 0 && n < ticked { "Convert the ticked files that are not done yet" } else { "Start conversion" };
                    let b = egui::Button::new(RichText::new(label).size(18.0).strong().color(Color32::WHITE)).fill(ACCENT).min_size(egui::vec2(170.0, 44.0));
                    if ui.add(b).on_hover_text(hint).clicked() {
                        self.start_conversion();
                    }
                }
                ui.checkbox(&mut self.shutdown_after, "Shut down when done").on_hover_text("Turns the computer off after the last file. Not when you press Stop.");
            });
        });
        ui.horizontal(|ui| {
            if self.converting {
                // Only the files of this conversion count, by their (trimmed) length.
                let parts: Vec<(f64, f32)> = self
                    .tasks
                    .iter()
                    .filter(|t| self.batch.contains(&t.id))
                    .map(|t| {
                        let p = match t.status {
                            Status::Queued | Status::Ready => 0.0,
                            Status::Running => t.result.as_ref().map(|r| r.progress).unwrap_or(0.0),
                            _ => 1.0,
                        };
                        (t.edits.effective_duration(t.duration()), p)
                    })
                    .collect();
                let frac = overall_progress(&parts);
                self.overall = frac;
                let speed: f32 = self.tasks.iter().filter(|t| t.status == Status::Running).filter_map(|t| t.result.as_ref().map(|r| r.speed)).sum();
                let elapsed = self.started_at.map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0);
                ui.add(egui::ProgressBar::new(frac).desired_width((ui.available_width() - 250.0).max(100.0)).text(format!("{:.0}%   ·   {:.1}× realtime", frac * 100.0, speed)));
                let mut clock = format!("elapsed {}", engine::fmt_time(elapsed));
                if let Some(left) = time_left(elapsed, frac) {
                    clock.push_str(&format!("  ·  about {} left", engine::fmt_time(left)));
                }
                ui.label(clock);
            } else if let Some((m, c)) = self.message.clone() {
                colored(ui, &m, c);
                if self.shutdown_pending && ui.button("Cancel shutdown").on_hover_text("Keep the computer on").clicked() {
                    self.cancel_shutdown();
                }
            } else {
                ui.label(RichText::new(format!("{} conversion profiles · drag files, folders or a subtitle file onto the window", self.library.len())).weak());
            }
        });
        ui.add_space(4.0);
    }

    fn preview_panel(&mut self, ui: &mut egui::Ui) {
        ui.add_space(6.0);
        let w = ui.available_width();
        let h = w * 9.0 / 16.0;
        let (rect, _) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 8.0, Color32::from_rgb(12, 14, 18));
        let sel = self.selected.and_then(|id| self.task(id)).map(|t| (t.id, t.info.clone(), t.path.clone()));
        let mut drew = false;
        if let (Some((id, _, _)), Some((tid, tex))) = (&sel, &self.preview_tex) {
            if id == tid {
                let size = tex.size_vec2();
                let scale = (rect.width() / size.x).min(rect.height() / size.y);
                let r = egui::Rect::from_center_size(rect.center(), size * scale);
                painter.image(tex.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
                drew = true;
            }
        }
        if !drew {
            for i in 0..12 {
                let t = i as f32 / 12.0;
                let c = Color32::from_rgb((20.0 + 22.0 * t) as u8, (26.0 + 34.0 * t) as u8, (40.0 + 64.0 * t) as u8);
                let y0 = rect.top() + rect.height() * t;
                painter.rect_filled(egui::Rect::from_min_max(egui::pos2(rect.left(), y0), egui::pos2(rect.right(), y0 + rect.height() / 12.0 + 1.0)), 0.0, c);
            }
            let msg = match &sel {
                Some((_, Some(i), _)) if !i.has_video => "🎵  Audio file",
                Some((_, None, _)) => "Reading file…",
                Some(_) => "Loading preview…",
                None => "🎬  Preview",
            };
            painter.text(rect.center(), egui::Align2::CENTER_CENTER, msg, egui::FontId::proportional(18.0), Color32::from_white_alpha(170));
        }
        let Some((id, info, path)) = sel else {
            ui.add_space(8.0);
            ui.label(RichText::new("Select a file to see its picture, details and tracks.").weak());
            ui.add_space(12.0);
            ui.label(RichText::new("How it works").strong());
            ui.label("1.  Add videos or music (or drop them on the window)");
            ui.label("2.  Pick a format – search by device, site or codec");
            ui.label("3.  Press Convert");
            ui.add_space(8.0);
            let rep = self.gpu.lock().unwrap().clone();
            ui.label(RichText::new(rep.headline()).color(if rep.encoders.is_empty() { Color32::GRAY } else { GOOD }));
            return;
        };
        let Some(info) = info else { return };
        let dur = info.duration.max(0.0);
        ui.horizontal(|ui| {
            ui.style_mut().spacing.slider_width = (w - 110.0).max(60.0);
            let r = ui.add(egui::Slider::new(&mut self.preview_time, 0.0..=dur.max(0.1)).show_value(false));
            if r.drag_stopped() || (r.changed() && !r.dragged()) {
                self.request_preview();
            }
            ui.label(format!("{} / {}", engine::fmt_time(self.preview_time), engine::fmt_time(dur)));
        });
        ui.horizontal(|ui| {
            if ui.button("▶ Play").on_hover_text("Open in your video player").clicked() {
                open_path(&path);
            }
            if ui.add_enabled(info.has_video, egui::Button::new("📷 Snapshot")).on_hover_text("Save this moment as a picture").clicked() {
                let _ = std::fs::create_dir_all(&self.settings.snapshot_dir);
                let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
                let ext = if self.settings.snapshot_jpg { "jpg" } else { "png" };
                let out = self.settings.snapshot_dir.join(format!("{} {}.{}", stem, engine::fmt_time(self.preview_time).replace(':', "-"), ext));
                match engine::save_snapshot(&path, self.preview_time, &out, self.settings.snapshot_deinterlace) {
                    Ok(()) => self.message = Some((format!("Snapshot saved: {}", out.display()), GOOD)),
                    Err(e) => self.message = Some((format!("Snapshot failed: {}", e), BAD)),
                }
            }
            if ui.button("📂").on_hover_text("Show in folder").clicked() {
                reveal(&path);
            }
        });
        ui.separator();
        let est = self.task(id).and_then(|t| engine::estimate_output(&self.current, &info, &t.edits, t.audio_track));
        egui::Grid::new("props").num_columns(2).spacing([10.0, 4.0]).show(ui, |ui| {
            let row = |ui: &mut egui::Ui, k: &str, v: String| {
                ui.label(RichText::new(k).weak());
                bidi::body(ui, &v);
                ui.end_row();
            };
            row(ui, "Duration", engine::fmt_time(dur));
            if info.has_video {
                row(ui, "Video", format!("{} · {}×{} · {} fps", info.vcodec, info.width, info.height, profiles::trim_float((info.fps * 100.0).round() / 100.0)));
            }
            if !info.acodec.is_empty() {
                row(ui, "Audio", info.audio.first().map(|a| a.label(0)).unwrap_or_default());
            }
            row(ui, "Size", format!("{}  ({})", engine::fmt_bytes(info.size), engine::fmt_bitrate(info.bitrate)));
            if let Some(est) = est {
                row(ui, "Output ≈", format!("{} (estimate)", engine::fmt_bytes(est)));
            }
        });
        ui.separator();
        let (mut atrack, mut sub, mut split, mut forced, mut sub_size) = match self.task(id) {
            Some(t) => (t.audio_track, t.subtitle.clone(), t.split_minutes, t.forced_only, t.sub_size),
            None => return,
        };
        let sub_before = (sub.clone(), forced, sub_size);
        egui::Grid::new("tracks").num_columns(2).spacing([10.0, 6.0]).show(ui, |ui| {
            ui.label("Audio track");
            let audio_text = info.audio.get(atrack).map(|a| a.label(atrack)).unwrap_or_else(|| "None".into());
            let combo = egui::ComboBox::from_id_salt("atrack").width((w - 110.0).max(80.0)).selected_text(bidi::egui_text(&audio_text)).show_ui(ui, |ui| {
                for (i, a) in info.audio.iter().enumerate() {
                    if bidi::selectable(ui, atrack == i, &a.label(i)).clicked() {
                        atrack = i;
                    }
                }
            });
            bidi::over(ui, &combo.response, &audio_text, 8.0, 26.0);
            ui.end_row();
            ui.label("Subtitles");
            let sub_text = match &sub {
                SubChoice::None => "Off".to_string(),
                SubChoice::Burn(i) => format!("Burn in: {}", info.subs.get(*i).map(|s| s.label(*i)).unwrap_or_default()),
                SubChoice::External(p) => format!("Burn in: {}", file_name(p)),
            };
            let combo = egui::ComboBox::from_id_salt("subs").width((w - 110.0).max(80.0)).selected_text(bidi::egui_text(&sub_text)).show_ui(ui, |ui| {
                ui.selectable_value(&mut sub, SubChoice::None, "Off");
                for (i, s) in info.subs.iter().enumerate() {
                    let kind = if s.text_based { "" } else { "  (picture)" };
                    if bidi::selectable(ui, sub == SubChoice::Burn(i), &format!("Burn in: {}{}", s.label(i), kind)).clicked() {
                        sub = SubChoice::Burn(i);
                    }
                }
                if ui.button("Load subtitle file (.srt / .ass / .ssa / .vtt)…").clicked() {
                    if let Some(f) = rfd::FileDialog::new().add_filter("Subtitles", SUBTITLE_EXT).pick_file() {
                        sub = SubChoice::External(f);
                    }
                }
            });
            bidi::over(ui, &combo.response, &sub_text, 8.0, 26.0);
            ui.end_row();
            if info.subs.is_empty() && sub == SubChoice::None {
                ui.label("");
                ui.label(RichText::new("No subtitles inside this file – load or drop a subtitle file.").weak().size(11.0));
                ui.end_row();
            }
            let picture = matches!(sub, SubChoice::Burn(i) if info.subs.get(i).map(|s| !s.text_based).unwrap_or(false));
            if picture {
                ui.label("");
                ui.checkbox(&mut forced, "Forced subtitles only").on_hover_text("Only the lines the disc marks as forced, e.g. foreign-language dialogue");
                ui.end_row();
            } else if sub != SubChoice::None {
                ui.label("Text size");
                ui.horizontal(|ui| {
                    for (v, l) in [(0u32, "As in file"), (18, "Small"), (24, "Medium"), (32, "Large")] {
                        ui.selectable_value(&mut sub_size, v, l);
                    }
                });
                ui.end_row();
            }
            ui.label("Split");
            ui.horizontal(|ui| {
                let mut on = split > 0.0;
                if ui.checkbox(&mut on, "every").changed() {
                    split = if on { 10.0 } else { 0.0 };
                }
                if on {
                    ui.add(egui::DragValue::new(&mut split).range(0.5..=600.0).speed(0.5).suffix(" min"));
                } else {
                    ui.label(RichText::new("off").weak());
                }
            });
            ui.end_row();
        });
        let sub_changed = sub_before != (sub.clone(), forced, sub_size);
        if sub_before.0 != sub {
            if let Some(t0) = engine::first_subtitle_time(&path, &sub) {
                self.preview_time = (t0 + 0.4).min(dur);
            }
        }
        if let Some(t) = self.task_mut(id) {
            t.audio_track = atrack;
            t.subtitle = sub;
            t.split_minutes = split;
            t.forced_only = forced;
            t.sub_size = sub_size;
        }
        if sub_changed {
            self.preview_key = None;
        }
    }

    fn empty_state(&mut self, ui: &mut egui::Ui) {
        let rect = ui.available_rect_before_wrap().shrink(12.0);
        let hovering = ui.ctx().input(|i| !i.raw.hovered_files.is_empty());
        let stroke_col = if hovering { ACCENT } else { Color32::from_gray(80) };
        ui.painter().rect(rect, 14.0, if hovering { ACCENT.gamma_multiply(0.08) } else { Color32::TRANSPARENT }, egui::Stroke::new(2.0_f32, stroke_col), egui::StrokeKind::Inside);
        ui.scope_builder(egui::UiBuilder::new().max_rect(rect), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(rect.height() * 0.2);
                ui.label(RichText::new("🎬").size(54.0));
                ui.label(RichText::new("Drop videos or music here").size(24.0).strong());
                ui.label(RichText::new("or use the buttons below — then pick a format and press Convert").weak());
                ui.add_space(16.0);
                ui.horizontal(|ui| {
                    let bw = 150.0;
                    ui.add_space(((ui.available_width() - bw * 2.0 - 10.0) / 2.0).max(0.0));
                    if ui.add(egui::Button::new(RichText::new("➕ Add files").size(15.0).color(Color32::WHITE)).min_size(egui::vec2(bw, 36.0)).fill(ACCENT)).clicked() {
                        self.pick_files();
                    }
                    if ui.add(egui::Button::new(RichText::new("📁 Add folder").size(15.0)).min_size(egui::vec2(bw, 36.0))).clicked() {
                        self.pick_folder();
                    }
                });
                ui.add_space(22.0);
                ui.label(RichText::new("MP4 · MKV · MOV · AVI · WMV · WebM · MPG · TS · FLV · 3GP · MP3 · AAC · FLAC · WAV · and more").weak().size(12.0));
                if engine::ffmpeg_path().is_none() {
                    ui.add_space(6.0);
                    ui.label(RichText::new("⚠ FFmpeg was not found. Put ffmpeg.exe and ffprobe.exe next to this program, or install FFmpeg.").color(WARN));
                }
            });
        });
    }

    fn task_list(&mut self, ui: &mut egui::Ui) {
        if self.tasks.is_empty() {
            self.empty_state(ui);
            return;
        }
        let mut action: Option<&str> = None;
        ui.horizontal(|ui| {
            let mut all = self.tasks.iter().all(|t| t.checked);
            if ui.checkbox(&mut all, "").on_hover_text("Tick / untick all").changed() {
                for t in self.tasks.iter_mut() {
                    t.checked = all;
                }
            }
            ui.label(RichText::new(format!("{} files", self.tasks.len())).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let idle = !self.converting;
                if ui.add_enabled(idle, egui::Button::new("🗑 Clear")).on_hover_text("Clear task list").on_disabled_hover_text("Not while converting").clicked() {
                    action = Some("clear");
                }
                let any_done = self.tasks.iter().any(|t| t.status == Status::Done);
                if ui.add_enabled(any_done, egui::Button::new("✔ Clear done")).on_hover_text("Take the converted files off the list").clicked() {
                    action = Some("clear_done");
                }
                if ui.button("✖ Remove").on_hover_text("Remove the selected file from the list (Delete)").clicked() {
                    action = Some("remove");
                }
                if ui.button("⬇").on_hover_text("Move down selected task").clicked() {
                    action = Some("down");
                }
                if ui.button("⬆").on_hover_text("Move up selected task").clicked() {
                    action = Some("up");
                }
            });
        });
        ui.separator();
        let mut select: Option<u64> = None;
        let mut edit: Option<u64> = None;
        let mut remove: Option<u64> = None;
        let mut log: Option<u64> = None;
        let mut cancel: Option<u64> = None;
        let profile_ext = self.current.ext.to_uppercase();
        let selected_id = self.selected;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            for t in self.tasks.iter_mut() {
                let selected = selected_id == Some(t.id);
                let fill = if selected { ACCENT.gamma_multiply(0.22) } else { ui.visuals().faint_bg_color };
                let resp = egui::Frame::new().fill(fill).corner_radius(8).inner_margin(egui::Margin::symmetric(10, 8)).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.set_min_height(38.0);
                        ui.checkbox(&mut t.checked, "");
                        let is_audio = t.info.as_ref().map(|i| !i.has_video).unwrap_or(false);
                        ui.label(RichText::new(if is_audio { "🎵" } else { "🎞" }).size(22.0));
                        let line = {
                            let line = match &t.info {
                                None => "Reading…".to_string(),
                                Some(i) if !i.ok => "Unreadable file".to_string(),
                                Some(i) => {
                                    let mut s = engine::fmt_time(i.duration);
                                    if i.has_video {
                                        s.push_str(&format!("  ·  {}×{}  ·  {}", i.width, i.height, i.vcodec));
                                    }
                                    if !i.acodec.is_empty() {
                                        s.push_str(&format!("  ·  {}", i.acodec));
                                    }
                                    s.push_str(&format!("  ·  {}", engine::fmt_bytes(i.size)));
                                    if !t.edits.is_default() {
                                        s.push_str("  ·  ✂ edited");
                                    }
                                    if t.subtitle != SubChoice::None {
                                        s.push_str("  ·  💬 subtitles");
                                    }
                                    if t.split_minutes > 0.0 {
                                        s.push_str(&format!("  ·  split every {} min", profiles::trim_float(t.split_minutes as f32)));
                                    }
                                    s
                                }
                            };
                            line
                        };
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("✖").on_hover_text("Remove").clicked() {
                                remove = Some(t.id);
                            }
                            if ui.small_button("✂").on_hover_text("Edit selected task").clicked() {
                                edit = Some(t.id);
                            }
                            let st = t.result.clone().unwrap_or_default();
                            match t.status {
                                Status::Ready => {
                                    ui.label(RichText::new(format!("→ {}", profile_ext)).color(ACCENT));
                                }
                                Status::Queued => {
                                    ui.label(RichText::new("Waiting…").weak());
                                }
                                Status::Running => {
                                    if ui.small_button("⏹").on_hover_text("Cancel this file").clicked() {
                                        cancel = Some(t.id);
                                    }
                                    let txt = if st.speed > 0.0 {
                                        format!("{:.0}%  {:.1}×  {} left", st.progress * 100.0, st.speed, engine::fmt_time(st.eta_secs))
                                    } else {
                                        format!("{:.0}%", st.progress * 100.0)
                                    };
                                    ui.add(egui::ProgressBar::new(st.progress).desired_width(200.0).text(txt));
                                    if !st.engine.is_empty() {
                                        ui.label(RichText::new(&st.engine).size(11.0).color(if st.engine.starts_with("GPU") { GOOD } else { Color32::GRAY }));
                                    }
                                }
                                Status::Done => {
                                    if ui.small_button("📂").on_hover_text("Show the converted file").clicked() {
                                        if let Some(o) = &t.output {
                                            reveal(o);
                                        }
                                    }
                                    let mut s = format!("✔ Done  {}  in {}", engine::fmt_bytes(st.out_bytes), engine::fmt_time(st.elapsed));
                                    if st.fell_back {
                                        s.push_str(" (CPU)");
                                    } else if st.engine.starts_with("GPU") {
                                        s.push_str(" ⚡");
                                    }
                                    if ui.add(egui::Label::new(RichText::new(s).color(GOOD)).sense(egui::Sense::click())).on_hover_text("Click for the conversion log").clicked() {
                                        log = Some(t.id);
                                    }
                                }
                                Status::Failed => {
                                    if ui.add(egui::Label::new(RichText::new("✖ Failed – details").color(BAD)).sense(egui::Sense::click())).on_hover_text(st.error.clone()).clicked() {
                                        log = Some(t.id);
                                    }
                                }
                                Status::Skipped => {
                                    ui.label(RichText::new("Skipped (file exists)").color(WARN));
                                }
                                Status::Cancelled => {
                                    ui.label(RichText::new("Cancelled").color(WARN));
                                }
                            }
                            // The name gets the width that is left and is cut with "…"
                            // rather than running into the status.
                            ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                                ui.spacing_mut().item_spacing.y = 2.0;
                                let strong = ui.visuals().strong_text_color();
                                bidi::label(ui, &t.name, 14.0, strong);
                                ui.add(egui::Label::new(RichText::new(line).weak().size(12.0)).truncate());
                            });
                        });
                    });
                });
                if selected && self.scroll_to_selected {
                    self.scroll_to_selected = false;
                    resp.response.scroll_to_me(None);
                }
                let row = resp.response.interact(egui::Sense::click());
                if row.clicked() {
                    select = Some(t.id);
                }
                if row.double_clicked() {
                    edit = Some(t.id);
                }
                row.context_menu(|ui| {
                    if ui.button("✂  Edit…").clicked() {
                        edit = Some(t.id);
                        ui.close_menu();
                    }
                    if ui.button("▶  Play the original").clicked() {
                        open_path(&t.path);
                        ui.close_menu();
                    }
                    if ui.button("📂  Show the original in its folder").clicked() {
                        reveal(&t.path);
                        ui.close_menu();
                    }
                    if let (Status::Done, Some(o)) = (t.status, &t.output) {
                        if ui.button("📂  Show the converted file").clicked() {
                            reveal(o);
                            ui.close_menu();
                        }
                    }
                    if t.result.is_some() && ui.button("📄  Conversion log").clicked() {
                        log = Some(t.id);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.add_enabled(t.job.is_none() && t.status != Status::Running, egui::Button::new("✖  Remove from the list")).clicked() {
                        remove = Some(t.id);
                        ui.close_menu();
                    }
                });
                ui.add_space(4.0);
            }
        });
        if let Some(id) = select {
            self.select(id);
        }
        if let Some(id) = edit {
            self.open_edit(id);
        }
        if let Some(id) = log {
            self.log_task = Some(id);
        }
        if let Some(id) = cancel {
            if let Some(j) = self.task(id).and_then(|t| t.job.as_ref()) {
                j.cancel();
            }
        }
        let sel_idx = self.selected.and_then(|id| self.tasks.iter().position(|t| t.id == id));
        match action {
            Some("clear") => {
                if !self.converting {
                    self.tasks.clear();
                    self.selected = None;
                    self.preview_tex = None;
                }
            }
            Some("clear_done") => {
                self.tasks.retain(|t| t.status != Status::Done || t.job.is_some());
                if self.selected.map(|id| self.task(id).is_none()).unwrap_or(false) {
                    self.selected = None;
                    self.preview_tex = None;
                    if let Some(first) = self.tasks.first().map(|t| t.id) {
                        self.select(first);
                    }
                }
            }
            Some("remove") => remove = self.selected,
            Some("up") => {
                if let Some(i) = sel_idx {
                    if i > 0 {
                        self.tasks.swap(i, i - 1);
                    }
                }
            }
            Some("down") => {
                if let Some(i) = sel_idx {
                    if i + 1 < self.tasks.len() {
                        self.tasks.swap(i, i + 1);
                    }
                }
            }
            _ => {}
        }
        if let Some(id) = remove {
            self.remove_task(id);
        }
    }

    // ------------------------------------------------------------ windows

    fn picker_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_picker;
        let mut chosen: Option<Profile> = None;
        let mut toggle_fav: Option<u32> = None;
        let mut delete: Option<u32> = None;
        let screen = ctx.screen_rect();
        let win_w = (screen.width() - 80.0).clamp(560.0, 940.0);
        let win_h = (screen.height() - 160.0).clamp(360.0, 620.0);
        egui::Window::new("Choose a format").open(&mut open).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]).fixed_size([win_w, win_h]).show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("🔍");
                ui.add(egui::TextEdit::singleline(&mut self.picker_search).hint_text("Search: iPhone, 4K, MP3, YouTube, HEVC…").desired_width(400.0));
                if !self.picker_search.is_empty() && ui.button("✖").clicked() {
                    self.picker_search.clear();
                }
                ui.label(RichText::new(format!("{} profiles", self.library.len())).weak());
            });
            ui.separator();
            let list_h = ui.available_height().max(200.0);
            ui.horizontal_top(|ui| {
                ui.allocate_ui_with_layout(egui::vec2(230.0, list_h), egui::Layout::top_down_justified(egui::Align::LEFT), |ui| {
                    egui::ScrollArea::vertical().id_salt("cats").auto_shrink([false, false]).show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        for special in ["Recent", "Favorites"] {
                            let n = if special == "Recent" { self.settings.recent.len() } else { self.settings.favorites.len() };
                            let icon = if special == "Recent" { "🕘" } else { "♥" };
                            if ui.selectable_label(self.picker_cat == special && self.picker_search.is_empty(), format!("{} {}  ({})", icon, special, n)).clicked() {
                                self.picker_cat = special.into();
                                self.picker_search.clear();
                            }
                        }
                        ui.separator();
                        for (cat, icon) in CATEGORIES {
                            let n = self.library.iter().filter(|p| p.category == *cat).count();
                            if n == 0 && *cat == "My Profiles" {
                                continue;
                            }
                            if ui.selectable_label(self.picker_cat == *cat && self.picker_search.is_empty(), format!("{} {}  ({})", icon, cat, n)).clicked() {
                                self.picker_cat = cat.to_string();
                                self.picker_search.clear();
                            }
                        }
                    });
                });
                ui.separator();
                let q = self.picker_search.to_lowercase();
                let words: Vec<&str> = q.split_whitespace().collect();
                let list: Vec<Profile> = if !words.is_empty() {
                    self.library
                        .iter()
                        .filter(|p| {
                            let hay = format!("{} {} {} {} {}", p.name, p.category, p.ext, p.summary(), p.note).to_lowercase();
                            words.iter().all(|w| hay.contains(w))
                        })
                        .cloned()
                        .collect()
                } else if self.picker_cat == "Recent" {
                    self.settings.recent.iter().filter_map(|id| self.library.iter().find(|p| p.id == *id).cloned()).collect()
                } else if self.picker_cat == "Favorites" {
                    self.settings.favorites.iter().filter_map(|id| self.library.iter().find(|p| p.id == *id).cloned()).collect()
                } else {
                    self.library.iter().filter(|p| p.category == self.picker_cat).cloned().collect()
                };
                let right_w = ui.available_width();
                ui.allocate_ui_with_layout(egui::vec2(right_w, list_h), egui::Layout::top_down(egui::Align::LEFT), |ui| {
                    egui::ScrollArea::vertical().id_salt("plist").auto_shrink([false, false]).show(ui, |ui| {
                        ui.set_width(right_w - 16.0);
                        if list.is_empty() {
                            ui.label(RichText::new("Nothing here yet.").weak());
                        }
                        for p in &list {
                            let current = p.id == self.current.id;
                            let fill = if current { ACCENT.gamma_multiply(0.25) } else { ui.visuals().faint_bg_color };
                            let r = egui::Frame::new().fill(fill).corner_radius(6).inner_margin(egui::Margin::symmetric(10, 6)).show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    let fav = self.settings.favorites.contains(&p.id);
                                    if ui
                                        .add(egui::Button::new(RichText::new(if fav { "♥" } else { "♡" }).color(if fav { BAD } else { Color32::GRAY })).frame(false))
                                        .on_hover_text("Favourite")
                                        .clicked()
                                    {
                                        toggle_fav = Some(p.id);
                                    }
                                    ui.vertical(|ui| {
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(&p.name).strong());
                                            ui.label(RichText::new(format!(".{}", p.ext)).color(ACCENT).size(11.0));
                                            if !self.picker_search.is_empty() {
                                                ui.label(RichText::new(&p.category).weak().size(11.0));
                                            }
                                        });
                                        ui.label(RichText::new(p.summary()).weak().size(11.5));
                                        if !p.note.is_empty() {
                                            ui.label(RichText::new(&p.note).size(11.5).italics());
                                        }
                                    });
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if p.custom && ui.small_button("🗑").on_hover_text("Delete this profile").clicked() {
                                            delete = Some(p.id);
                                        }
                                        if p.vcodec.gpu_capable() {
                                            ui.label(RichText::new("⚡GPU").size(11.0).color(GOOD)).on_hover_text("Can be converted on the graphics card");
                                        }
                                    });
                                });
                            });
                            if r.response.interact(egui::Sense::click()).clicked() {
                                chosen = Some(p.clone());
                            }
                            ui.add_space(3.0);
                        }
                    });
                });
            });
        });
        if let Some(id) = toggle_fav {
            if let Some(i) = self.settings.favorites.iter().position(|x| *x == id) {
                self.settings.favorites.remove(i);
            } else {
                self.settings.favorites.push(id);
            }
            self.save_settings();
        }
        if let Some(id) = delete {
            self.library.retain(|p| p.id != id);
            self.settings.custom_profiles.retain(|p| p.id != id);
            self.save_settings();
        }
        if let Some(p) = chosen {
            self.set_profile(p);
            open = false;
        }
        self.show_picker = open;
    }

    fn customize_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_customize;
        let mut save_new: Option<String> = None;
        let mut reset = false;
        // The file the comparison is about: the selected one, else the first.
        let task_info = self
            .selected
            .and_then(|id| self.task(id))
            .or_else(|| self.tasks.first())
            .and_then(|t| t.info.clone().filter(|i| i.ok).map(|i| (t.name.clone(), t.path.clone(), i, t.edits.clone(), t.audio_track)));
        let hw_name = {
            let (enc, _) = self.hw_for(&self.current);
            let rep = self.gpu.lock().unwrap();
            enc.map(|e| rep.adapter_for(e.vendor).map(|a| a.name.clone()).unwrap_or_else(|| e.vendor.short().into()))
        };
        egui::Window::new("Customize format").open(&mut open).collapsible(false).default_width(980.0).show(ctx, |ui| {
            let p = &mut self.current;
            ui.label(RichText::new(&p.name).strong().size(15.0));
            ui.label(RichText::new("Changes apply to the next conversion. Save them to keep them in “My Profiles”.").weak());
            ui.separator();
            ui.horizontal_top(|ui| {
                ui.vertical(|ui| {
                    ui.set_max_width(470.0);
                    egui::Grid::new("cust").num_columns(2).spacing([14.0, 8.0]).show(ui, |ui| {
                        ui.label("File type");
                        egui::ComboBox::from_id_salt("ext").selected_text(format!(".{}", p.ext)).show_ui(ui, |ui| {
                            for e in [
                                "mp4", "mkv", "mov", "m4v", "avi", "webm", "wmv", "ts", "m2ts", "mpg", "flv", "3gp", "ogv", "gif", "mp3", "m4a", "aac", "flac", "wav", "opus", "ogg", "ac3", "wma",
                                "aiff",
                            ] {
                                ui.selectable_value(&mut p.ext, e.to_string(), format!(".{}", e));
                            }
                        });
                        ui.end_row();
                        ui.label(RichText::new("Video").strong());
                        ui.end_row();
                        ui.label("Codec");
                        egui::ComboBox::from_id_salt("vc").selected_text(p.vcodec.label()).show_ui(ui, |ui| {
                            for v in VCodec::ALL {
                                let lbl = if v.gpu_capable() { format!("{}  ⚡", v.label()) } else { v.label().to_string() };
                                ui.selectable_value(&mut p.vcodec, v, lbl);
                            }
                        });
                        ui.end_row();
                        let video = !matches!(p.vcodec, VCodec::None | VCodec::Copy);
                        ui.label("Size");
                        ui.add_enabled_ui(video, |ui| {
                            ui.horizontal(|ui| {
                                let label = if p.height == 0 && p.width == 0 {
                                    "Original".to_string()
                                } else if p.width == 0 {
                                    format!("{}p", p.height)
                                } else {
                                    format!("{}×{}", p.width, p.height)
                                };
                                egui::ComboBox::from_id_salt("size").selected_text(label).show_ui(ui, |ui| {
                                    for (w, h, l) in [
                                        (0, 0, "Original"),
                                        (0, 360, "360p"),
                                        (0, 480, "480p"),
                                        (0, 720, "720p HD"),
                                        (0, 1080, "1080p Full HD"),
                                        (0, 1440, "1440p"),
                                        (0, 2160, "4K"),
                                        (0, 4320, "8K"),
                                        (1080, 1920, "1080×1920 vertical"),
                                        (1080, 1080, "1080×1080 square"),
                                    ] {
                                        if ui.selectable_label(p.width == w && p.height == h, l).clicked() {
                                            p.width = w;
                                            p.height = h;
                                            p.fit = if w == 0 { Fit::Height } else { Fit::Pad };
                                        }
                                    }
                                });
                                ui.add(egui::DragValue::new(&mut p.width).range(0..=8192).prefix("W "));
                                ui.add(egui::DragValue::new(&mut p.height).range(0..=8192).prefix("H "));
                            });
                        });
                        ui.end_row();
                        ui.label("Fit");
                        ui.add_enabled_ui(video && p.width > 0 && p.height > 0, |ui| {
                            ui.horizontal(|ui| {
                                ui.radio_value(&mut p.fit, Fit::Pad, "Fit (black bars)");
                                ui.radio_value(&mut p.fit, Fit::Fill, "Fill (crop)");
                            });
                        });
                        ui.end_row();
                        ui.label("Frame rate");
                        ui.add_enabled_ui(video, |ui| {
                            egui::ComboBox::from_id_salt("fps").selected_text(if p.fps == 0.0 { "Original".into() } else { format!("{} fps", profiles::trim_float(p.fps)) }).show_ui(ui, |ui| {
                                for f in [0.0, 12.0, 15.0, 23.976, 24.0, 25.0, 29.97, 30.0, 50.0, 59.94, 60.0, 120.0] {
                                    ui.selectable_value(&mut p.fps, f, if f == 0.0 { "Original".into() } else { format!("{} fps", profiles::trim_float(f)) });
                                }
                            });
                        });
                        ui.end_row();
                        ui.label("Quality");
                        ui.add_enabled_ui(video, |ui| {
                            ui.horizontal(|ui| {
                                egui::ComboBox::from_id_salt("q").selected_text(p.quality.label()).show_ui(ui, |ui| {
                                    for q in Quality::ALL {
                                        ui.selectable_value(&mut p.quality, q, q.label());
                                    }
                                });
                                if p.quality == Quality::Bitrate {
                                    if p.vkbps == 0 {
                                        p.vkbps = 4000;
                                    }
                                    ui.add(egui::DragValue::new(&mut p.vkbps).range(100..=200000).speed(50).suffix(" kbps"));
                                }
                            });
                        });
                        ui.end_row();
                        ui.label("10-bit colour");
                        ui.add_enabled(video && matches!(p.vcodec, VCodec::H265 | VCodec::Av1 | VCodec::Vp9), egui::Checkbox::new(&mut p.ten_bit, "Smoother gradients / HDR"));
                        ui.end_row();
                        ui.label(RichText::new("Audio").strong());
                        ui.end_row();
                        ui.label("Codec");
                        egui::ComboBox::from_id_salt("ac").selected_text(p.acodec.label()).show_ui(ui, |ui| {
                            for a in ACodec::ALL {
                                ui.selectable_value(&mut p.acodec, a, a.label());
                            }
                        });
                        ui.end_row();
                        let audio = !matches!(p.acodec, ACodec::None | ACodec::Copy);
                        ui.label("Bitrate");
                        ui.add_enabled_ui(audio && !p.acodec.lossless(), |ui| {
                            egui::ComboBox::from_id_salt("ab").selected_text(if p.akbps == 0 { "Default".into() } else { format!("{} kbps", p.akbps) }).show_ui(ui, |ui| {
                                for k in [0u32, 32, 48, 64, 96, 128, 160, 192, 224, 256, 320, 384, 448, 640] {
                                    ui.selectable_value(&mut p.akbps, k, if k == 0 { "Default".into() } else { format!("{} kbps", k) });
                                }
                            });
                        });
                        ui.end_row();
                        ui.label("Sample rate");
                        ui.add_enabled_ui(audio, |ui| {
                            egui::ComboBox::from_id_salt("sr").selected_text(if p.sample_rate == 0 { "Original".into() } else { format!("{} Hz", p.sample_rate) }).show_ui(ui, |ui| {
                                for r in [0u32, 8000, 22050, 32000, 44100, 48000, 96000, 192000] {
                                    ui.selectable_value(&mut p.sample_rate, r, if r == 0 { "Original".into() } else { format!("{} Hz", r) });
                                }
                            });
                        });
                        ui.end_row();
                        ui.label("Channels");
                        ui.add_enabled_ui(audio, |ui| {
                            let text = match p.channels {
                                0 => "Original",
                                1 => "Mono",
                                2 => "Stereo",
                                6 => "5.1 Channels",
                                8 => "7.1 Channels",
                                _ => "?",
                            };
                            egui::ComboBox::from_id_salt("ch").selected_text(text).show_ui(ui, |ui| {
                                for (c, l) in [(0u8, "Original"), (1, "Mono"), (2, "Stereo"), (6, "5.1 Channels"), (8, "7.1 Channels")] {
                                    ui.selectable_value(&mut p.channels, c, l);
                                }
                            });
                        });
                        ui.end_row();
                    });
                });
                ui.add_space(18.0);
                ui.vertical(|ui| {
                    compare_table(ui, p, task_info.as_ref(), hw_name.clone());
                });
            });
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("💾 Save to My Profiles").clicked() {
                    save_new = Some(if p.custom { p.name.clone() } else { format!("{} (custom)", p.name) });
                }
                if ui.button("Reset to original").clicked() {
                    reset = true;
                }
            });
        });
        if reset {
            let id = self.current.id;
            if let Some(orig) = self.library.iter().find(|x| x.id == id) {
                self.current = orig.clone();
            }
        }
        if let Some(name) = save_new {
            let id = self.library.iter().map(|p| p.id).max().unwrap_or(0).max(900_000) + 1;
            let mut p = self.current.clone();
            p.id = id;
            p.name = name;
            p.custom = true;
            p.category = "My Profiles".into();
            self.library.push(p.clone());
            self.settings.custom_profiles.push(p.clone());
            self.set_profile(p);
            self.message = Some(("Saved to My Profiles".into(), GOOD));
        }
        self.show_customize = open;
    }

    fn options_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_options;
        let mut redetect = false;
        let mut theme_changed = false;
        let rep = self.gpu.lock().unwrap().clone();
        egui::Window::new("Options").open(&mut open).collapsible(false).default_size([620.0, 400.0]).show(ctx, |ui| {
            ui.horizontal(|ui| {
                for (t, l) in [(OptTab::General, "General"), (OptTab::Conversion, "Conversion"), (OptTab::Acceleration, "⚡ Acceleration"), (OptTab::Snapshot, "Snapshot")] {
                    ui.selectable_value(&mut self.opt_tab, t, RichText::new(l).size(14.0));
                }
            });
            ui.separator();
            let s = &mut self.settings;
            match self.opt_tab {
                OptTab::General => {
                    ui.label(RichText::new("If the output file already exists").strong());
                    ui.horizontal(|ui| {
                        ui.radio_value(&mut s.exists, ExistsPolicy::Rename, "Rename the new file");
                        ui.radio_value(&mut s.exists, ExistsPolicy::Overwrite, "Overwrite");
                        ui.radio_value(&mut s.exists, ExistsPolicy::Skip, "Skip");
                    });
                    ui.add_space(8.0);
                    ui.checkbox(&mut s.open_folder_after, "Open the output folder when conversion finishes");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label("Look:");
                        theme_changed |= ui.radio_value(&mut s.dark, true, "Dark").changed();
                        theme_changed |= ui.radio_value(&mut s.dark, false, "Light").changed();
                    });
                }
                OptTab::Conversion => {
                    ui.horizontal(|ui| {
                        ui.label("Files converted at the same time:");
                        ui.add(egui::Slider::new(&mut s.max_parallel, 1..=8));
                    });
                    ui.label(RichText::new("2 is a good choice. With a graphics card, 2–3 keeps it busy.").weak());
                    ui.add_space(8.0);
                    ui.label(RichText::new("Processor cores").strong());
                    ui.horizontal(|ui| {
                        if ui.radio(s.threads == 0, "Use all cores (fastest)").clicked() {
                            s.threads = 0;
                        }
                        if ui.radio(s.threads > 0, "Limit to").clicked() && s.threads == 0 {
                            s.threads = 2;
                        }
                        if s.threads > 0 {
                            let max = std::thread::available_parallelism().map(|n| n.get() as u32).unwrap_or(8);
                            ui.add(egui::Slider::new(&mut s.threads, 1..=max).suffix(" cores"));
                        }
                    });
                    ui.label(RichText::new("Limit cores to keep the computer responsive while converting.").weak());
                }
                OptTab::Acceleration => {
                    ui.label(RichText::new("Graphics cards found").strong());
                    if !rep.done {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label("Testing your graphics cards…");
                        });
                    } else if rep.adapters.is_empty() {
                        ui.label("None found.");
                    }
                    for a in &rep.adapters {
                        let encs: Vec<String> = rep.encoders.iter().filter(|e| e.vendor == a.vendor).map(|e| e.codec.label().split(' ').next().unwrap_or("").to_string()).collect();
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(if encs.is_empty() { "○" } else { "●" }).color(if encs.is_empty() { Color32::GRAY } else { GOOD }));
                            ui.label(RichText::new(&a.name).strong());
                            if a.discrete {
                                ui.label(RichText::new("external / dedicated").size(11.0).color(ACCENT));
                            }
                            if encs.is_empty() {
                                ui.label(RichText::new("– no hardware encoder available").weak());
                            } else {
                                ui.label(RichText::new(format!("– encodes {}", encs.join(", "))).color(GOOD));
                            }
                        });
                    }
                    if ui.button("🔄 Test again").clicked() {
                        redetect = true;
                    }
                    ui.separator();
                    ui.checkbox(&mut s.gpu_encode, "Enable GPU encode acceleration").on_hover_text("H.264, H.265 and AV1 are encoded on the graphics card – usually several times faster");
                    ui.checkbox(&mut s.gpu_decode, "Enable GPU decode acceleration").on_hover_text("The graphics card reads the source video, freeing the processor");
                    ui.add_space(6.0);
                    ui.label(RichText::new("Which device").strong());
                    ui.radio_value(&mut s.accel, AccelChoice::Auto, "Automatic – prefer an external / dedicated graphics card");
                    for v in rep.vendors() {
                        let name = rep.adapter_for(v).map(|a| a.name.clone()).unwrap_or_else(|| v.label().into());
                        ui.radio_value(&mut s.accel, AccelChoice::Vendor(v), format!("{} – {}", v.label(), name));
                    }
                    ui.radio_value(&mut s.accel, AccelChoice::CpuOnly, "Processor only (no graphics card)");
                    ui.add_space(6.0);
                    ui.label(RichText::new("If the graphics card can't handle a file, it is converted on the processor automatically.").weak());
                }
                OptTab::Snapshot => {
                    ui.horizontal(|ui| {
                        ui.label("Save to:");
                        let mut p = s.snapshot_dir.to_string_lossy().to_string();
                        if bidi::text_edit(ui, &mut p, 320.0, "").changed() {
                            s.snapshot_dir = PathBuf::from(p);
                        }
                        if ui.button("Browse").clicked() {
                            if let Some(d) = rfd::FileDialog::new().pick_folder() {
                                s.snapshot_dir = d;
                            }
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Format:");
                        ui.radio_value(&mut s.snapshot_jpg, false, "PNG");
                        ui.radio_value(&mut s.snapshot_jpg, true, "JPG");
                    });
                    ui.checkbox(&mut s.snapshot_deinterlace, "Deinterlace snapshots");
                }
            }
        });
        if redetect {
            *self.gpu.lock().unwrap() = GpuReport::default();
            gpu::detect_async(self.gpu.clone(), ctx.clone());
        }
        if theme_changed {
            self.apply_theme(ctx);
        }
        if open != self.show_options || theme_changed {
            self.save_settings();
        }
        self.show_options = open;
    }

    fn edit_window(&mut self, ctx: &egui::Context) {
        let Some(id) = self.edit_task else { return };
        let Some(task) = self.task(id) else {
            self.edit_task = None;
            return;
        };
        let info = task.info.clone().unwrap_or_default();
        let mut edits = task.edits.clone();
        let (mut sub, mut forced, mut sub_size) = (task.subtitle.clone(), task.forced_only, task.sub_size);
        let sub_before = (sub.clone(), forced, sub_size);
        let task_path = task.path.clone();
        let mut jump_to_sub = false;
        // A window title is drawn by egui, which cannot join Kurdish or Arabic
        // letters: such a name is shown inside the window instead.
        let task_name = task.name.clone();
        let native_name = bidi::needs_native(&task_name);
        let title = if native_name { "Edit".to_string() } else { format!("Edit – {}", task_name) };
        let mut open = true;
        let mut apply_all = false;
        let mut want_preview = false;
        let tex = self.preview_tex.as_ref().filter(|(tid, _)| *tid == id).map(|(_, t)| t.clone());
        let mut preview_time = self.preview_time;
        let mut tab = self.edit_tab;
        egui::Window::new(title).id(egui::Id::new("edit_win")).open(&mut open).collapsible(false).default_size([720.0, 560.0]).show(ctx, |ui| {
            if native_name {
                let strong = ui.visuals().strong_text_color();
                colored(ui, &task_name, strong);
            }
            let h = ui.available_width().min(640.0) * 9.0 / 16.0;
            let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::hover());
            ui.painter().rect_filled(rect, 6.0, Color32::BLACK);
            if let Some(t) = &tex {
                let size = t.size_vec2();
                let sc = (rect.width() / size.x).min(rect.height() / size.y);
                let r = egui::Rect::from_center_size(rect.center(), size * sc);
                ui.painter().image(t.id(), r, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), Color32::WHITE);
            } else {
                ui.painter().text(rect.center(), egui::Align2::CENTER_CENTER, if info.has_video { "Loading…" } else { "🎵 Audio only" }, egui::FontId::proportional(16.0), Color32::GRAY);
            }
            let dur = info.duration.max(0.1);
            ui.horizontal(|ui| {
                ui.style_mut().spacing.slider_width = (ui.available_width() - 120.0).max(60.0);
                let r = ui.add(egui::Slider::new(&mut preview_time, 0.0..=dur).show_value(false));
                if r.drag_stopped() || (r.changed() && !r.dragged()) {
                    want_preview = true;
                }
                ui.label(format!("{} / {}", engine::fmt_time(preview_time), engine::fmt_time(dur)));
            });
            ui.separator();
            ui.horizontal(|ui| {
                for (t, l) in [
                    (EditTab::Trim, "✂ Trim"),
                    (EditTab::Crop, "⛶ Crop"),
                    (EditTab::Effects, "🎨 Effects"),
                    (EditTab::Watermark, "🅰 Text"),
                    (EditTab::Audio, "🔊 Audio"),
                    (EditTab::Subtitle, "💬 Subtitle"),
                ] {
                    ui.selectable_value(&mut tab, t, RichText::new(l).size(14.0));
                }
            });
            ui.add_space(4.0);
            let before = edits.clone();
            match tab {
                EditTab::Trim => {
                    ui.horizontal(|ui| {
                        ui.label("Start");
                        ui.add(egui::DragValue::new(&mut edits.trim_start).range(0.0..=dur).speed(0.1).custom_formatter(|v, _| engine::fmt_clock(v)).custom_parser(engine::parse_clock))
                            .on_hover_text("Drag, or click and type a time: 1:30, 0:01:30.5 or 90");
                        if ui.button("Set to current").clicked() {
                            edits.trim_start = preview_time;
                        }
                        ui.add_space(20.0);
                        ui.label("End");
                        let mut end = if edits.trim_end > 0.0 { edits.trim_end } else { dur };
                        let end_box = ui.add(egui::DragValue::new(&mut end).range(0.0..=dur).speed(0.1).custom_formatter(|v, _| engine::fmt_clock(v)).custom_parser(engine::parse_clock));
                        if end_box.on_hover_text("Drag, or click and type a time: 1:30, 0:01:30.5 or 90").changed() {
                            edits.trim_end = if end >= dur - 0.05 { 0.0 } else { end };
                        }
                        if ui.button("Set to current").clicked() {
                            edits.trim_end = preview_time;
                        }
                    });
                    if edits.trim_end > 0.0 && edits.trim_end <= edits.trim_start {
                        ui.label(RichText::new("The end must be after the start.").color(BAD));
                    }
                    ui.label(format!("Length after trimming: {}", engine::fmt_time(edits.effective_duration(dur))));
                    if ui.button("Reset trim").clicked() {
                        edits.trim_start = 0.0;
                        edits.trim_end = 0.0;
                    }
                }
                EditTab::Crop => {
                    ui.horizontal(|ui| {
                        for (i, l) in ["Left", "Top", "Right", "Bottom"].iter().enumerate() {
                            ui.label(*l);
                            ui.add(egui::DragValue::new(&mut edits.crop[i]).range(0..=4000).suffix(" px"));
                        }
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Quick:");
                        for (l, ratio) in [("16:9", 16.0 / 9.0), ("4:3", 4.0 / 3.0), ("1:1", 1.0), ("9:16", 9.0 / 16.0), ("2.35:1", 2.35)] {
                            if ui.button(l).on_hover_text("Centre crop to this shape").clicked() && info.width > 0 && info.height > 0 {
                                let (w, h) = (info.width as f64, info.height as f64);
                                if w / h > ratio {
                                    let side = ((w - (h * ratio).round()) / 2.0) as u32;
                                    edits.crop = [side, 0, side, 0];
                                } else {
                                    let side = ((h - (w / ratio).round()) / 2.0) as u32;
                                    edits.crop = [0, side, 0, side];
                                }
                            }
                        }
                        if ui.button("Remove letterbox bars").clicked() && info.height > 0 {
                            let bar = (info.height as f64 * 0.12) as u32;
                            edits.crop = [0, bar, 0, bar];
                        }
                        if ui.button("Reset").clicked() {
                            edits.crop = [0; 4];
                        }
                    });
                    if info.width > 0 {
                        let [l, t, r, b] = edits.crop;
                        ui.label(format!("Result: {}×{}", info.width.saturating_sub(l + r), info.height.saturating_sub(t + b)));
                    }
                }
                EditTab::Effects => {
                    ui.horizontal(|ui| {
                        ui.label("Rotate");
                        for r in [0u32, 90, 180, 270] {
                            ui.selectable_value(&mut edits.rotate, r, format!("{}°", r));
                        }
                        ui.checkbox(&mut edits.flip_h, "Mirror");
                        ui.checkbox(&mut edits.flip_v, "Upside down");
                    });
                    ui.checkbox(&mut edits.deinterlace, "Deinterlace (remove comb lines from old TV recordings)");
                    egui::Grid::new("fx").num_columns(2).show(ui, |ui| {
                        ui.label("Brightness");
                        ui.add(egui::Slider::new(&mut edits.brightness, -0.5..=0.5));
                        ui.end_row();
                        ui.label("Contrast");
                        ui.add(egui::Slider::new(&mut edits.contrast, 0.3..=2.0));
                        ui.end_row();
                        ui.label("Saturation");
                        ui.add(egui::Slider::new(&mut edits.saturation, 0.0..=3.0));
                        ui.end_row();
                    });
                    if ui.button("Reset effects").clicked() {
                        let d = Edits::default();
                        edits.rotate = d.rotate;
                        edits.flip_h = d.flip_h;
                        edits.flip_v = d.flip_v;
                        edits.deinterlace = d.deinterlace;
                        edits.brightness = d.brightness;
                        edits.contrast = d.contrast;
                        edits.saturation = d.saturation;
                    }
                }
                EditTab::Watermark => {
                    ui.horizontal(|ui| {
                        ui.label("Text");
                        bidi::text_edit(ui, &mut edits.watermark, 320.0, "e.g. My Holiday 2026");
                    });
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Position");
                        for (i, l) in ["Top left", "Top right", "Bottom left", "Bottom right", "Centre"].iter().enumerate() {
                            ui.selectable_value(&mut edits.watermark_pos, i as u8, *l);
                        }
                    });
                    ui.horizontal(|ui| {
                        ui.label("Size");
                        ui.add(egui::Slider::new(&mut edits.watermark_size, 10..=120));
                    });
                }
                EditTab::Audio => {
                    ui.horizontal(|ui| {
                        ui.label("Volume");
                        ui.add(egui::Slider::new(&mut edits.volume, 0.0..=400.0).suffix(" %"));
                    });
                    ui.checkbox(&mut edits.normalize, "Even out loudness (normalize)");
                }
                EditTab::Subtitle => {
                    let mut enabled = sub != SubChoice::None;
                    if ui.checkbox(&mut enabled, RichText::new("Enable – burn subtitles into the video").strong()).changed() {
                        if enabled {
                            if !info.subs.is_empty() {
                                sub = SubChoice::Burn(0);
                            } else if let Some(f) = rfd::FileDialog::new().add_filter("Subtitles", SUBTITLE_EXT).pick_file() {
                                sub = SubChoice::External(f);
                            }
                        } else {
                            sub = SubChoice::None;
                        }
                    }
                    ui.add_enabled_ui(enabled, |ui| {
                        if info.subs.is_empty() {
                            ui.label(RichText::new("This file has no subtitles inside – load a subtitle file.").weak());
                        } else {
                            ui.label("Subtitles inside this file:");
                            for (i, s) in info.subs.iter().enumerate() {
                                let kind = if s.text_based { "" } else { "  (picture)" };
                                let text = format!("{}{}", s.label(i), kind);
                                if bidi::needs_native(&text) {
                                    ui.horizontal(|ui| {
                                        ui.radio_value(&mut sub, SubChoice::Burn(i), "");
                                        bidi::body(ui, &text);
                                    });
                                } else {
                                    ui.radio_value(&mut sub, SubChoice::Burn(i), text);
                                }
                            }
                        }
                        ui.horizontal(|ui| {
                            if ui.button("📂 Load subtitle file (*.srt, *.ass/*.ssa, *.vtt)").clicked() {
                                if let Some(f) = rfd::FileDialog::new().add_filter("Subtitles", SUBTITLE_EXT).pick_file() {
                                    sub = SubChoice::External(f);
                                }
                            }
                            if let SubChoice::External(f) = &sub {
                                colored(ui, &file_name(f), ACCENT);
                            }
                        });
                        let picture = matches!(sub, SubChoice::Burn(i) if info.subs.get(i).map(|s| !s.text_based).unwrap_or(false));
                        if picture {
                            ui.checkbox(&mut forced, "Forced subtitles only");
                        } else if sub != SubChoice::None {
                            ui.horizontal(|ui| {
                                ui.label("Text size");
                                for (v, l) in [(0u32, "As in file"), (18, "Small"), (24, "Medium"), (32, "Large")] {
                                    ui.selectable_value(&mut sub_size, v, l);
                                }
                            });
                        }
                        if sub != SubChoice::None && ui.button("▶ Show the first subtitle in the preview").clicked() {
                            jump_to_sub = true;
                        }
                    });
                    ui.label(RichText::new("The preview above shows the subtitles exactly as they will be burned in.").weak().size(11.0));
                }
            }
            if edits.picture_filters() != before.picture_filters() || edits.watermark_filter() != before.watermark_filter() {
                want_preview = true;
            }
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Apply to all files").clicked() {
                    apply_all = true;
                }
                if ui.button("Reset everything").clicked() {
                    edits = Edits::default();
                    want_preview = true;
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Done").clicked() {
                        ui.ctx().data_mut(|d| d.insert_temp(egui::Id::new("edit_close"), true));
                    }
                });
            });
        });
        // A newly chosen subtitle: move the preview to its first line so it's visible.
        let sub_changed = sub_before != (sub.clone(), forced, sub_size);
        if (sub_changed && sub_before.0 != sub) || jump_to_sub {
            if let Some(t0) = engine::first_subtitle_time(&task_path, &sub) {
                preview_time = (t0 + 0.4).min(info.duration.max(0.0));
            }
        }
        if sub_changed || jump_to_sub {
            want_preview = true;
        }
        self.preview_time = preview_time;
        self.edit_tab = tab;
        if let Some(t) = self.task_mut(id) {
            t.edits = edits.clone();
            t.subtitle = sub;
            t.forced_only = forced;
            t.sub_size = sub_size;
        }
        if apply_all {
            for t in self.tasks.iter_mut() {
                let (ts, te) = (t.edits.trim_start, t.edits.trim_end);
                t.edits = edits.clone();
                if t.id != id {
                    t.edits.trim_start = ts;
                    t.edits.trim_end = te;
                }
            }
            self.message = Some(("Edits applied to all files (trim kept per file)".into(), GOOD));
        }
        if want_preview {
            self.request_preview();
        }
        let close = ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::new("edit_close")).unwrap_or(false));
        if !open || close {
            self.edit_task = None;
        }
    }

    fn log_window(&mut self, ctx: &egui::Context) {
        let Some(id) = self.log_task else { return };
        let Some(t) = self.task(id) else {
            self.log_task = None;
            return;
        };
        let st = t.result.clone().unwrap_or_default();
        let name = t.name.clone();
        let native_name = bidi::needs_native(&name);
        let title = if native_name { "Conversion log".to_string() } else { format!("Conversion log – {}", name) };
        let mut open = true;
        egui::Window::new(title).id(egui::Id::new("log_win")).open(&mut open).default_size([700.0, 420.0]).show(ctx, |ui| {
            if native_name {
                let strong = ui.visuals().strong_text_color();
                colored(ui, &name, strong);
            }
            if !st.error.is_empty() {
                colored(ui, &format!("Problem: {}", st.error), BAD);
            }
            if st.fell_back {
                ui.label(RichText::new("The graphics card couldn't convert this file, so the processor did it.").color(WARN));
            }
            ui.label(format!("Engine: {}", if st.engine.is_empty() { "-" } else { &st.engine }));
            if ui.button("📋 Copy log").clicked() {
                ui.ctx().copy_text(st.log.join("\n"));
            }
            ui.separator();
            egui::ScrollArea::vertical().stick_to_bottom(true).auto_shrink([false, false]).show(ui, |ui| {
                for l in &st.log {
                    if bidi::needs_native(l) {
                        let color = ui.visuals().text_color();
                        bidi::label(ui, l, 11.0, color);
                    } else {
                        ui.label(RichText::new(l).monospace().size(11.0));
                    }
                }
            });
        });
        if !open {
            self.log_task = None;
        }
    }

    fn about_window(&mut self, ctx: &egui::Context) {
        let mut open = self.show_about;
        egui::Window::new("About ReelForge").open(&mut open).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label(RichText::new(concat!("⚡ ReelForge ", env!("CARGO_PKG_VERSION"))).size(18.0).strong());
            ui.label("Fast video & audio converter with graphics-card acceleration.");
            ui.label(format!("{} conversion profiles in {} categories.", self.library.len(), CATEGORIES.len() - 1));
            let engine_line = match engine::ffmpeg_path() {
                Some(p) => format!("Engine: FFmpeg at {}", p.display()),
                None => "Engine: FFmpeg not found".into(),
            };
            bidi::body(ui, &engine_line);
            ui.label(RichText::new("Keys: Ctrl+O add files · ↑ ↓ select · Enter edit · Delete remove · Ctrl+A tick all · Esc close").weak().size(11.0));
            ui.separator();
            ui.horizontal(|ui| {
                ui.label("Developed by");
                ui.hyperlink_to("Zheer Khalid", "https://zheer.dev");
            });
            ui.hyperlink_to("Source on GitHub", "https://github.com/zheerkhalid/reelforge");
        });
        self.show_about = open;
    }

    fn screenshot_tick(&mut self, ctx: &egui::Context) {
        let Some(path) = self.screenshot.clone() else {
            return;
        };
        ctx.request_repaint();
        // Let the graphics-card check finish so the picture shows its result.
        let gpu_ready = self.gpu.lock().unwrap().done;
        // A minimised or zero-size window has no picture to read (the painter
        // would crash), so wait until the window really has a size.
        let (sized, minimized) = ctx.input(|i| {
            let v = i.viewport();
            let r = i.screen_rect();
            (r.width() >= 100.0 && r.height() >= 100.0 && v.inner_rect.map(|r| r.width() >= 100.0 && r.height() >= 100.0).unwrap_or(true), v.minimized.unwrap_or(false))
        });
        if self.frames + 1 == 120 && (!sized || minimized) {
            if self.wait_frames.is_multiple_of(30) {
                ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(false));
                ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(1280.0, 800.0)));
            }
            self.wait_frames += 1;
            if self.wait_frames > 3000 {
                eprintln!("screenshot: the window never got a size");
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            return;
        }
        if self.auto_convert > 0 && self.frames >= 60 && self.wait_frames < 20_000 {
            // Convert first, then take the picture of the result.
            self.wait_frames += 1;
            let read = !self.tasks.is_empty() && self.tasks.iter().all(|t| t.info.is_some());
            if self.auto_convert == 1 && read && gpu_ready {
                self.start_conversion();
                self.auto_convert = if self.converting { 2 } else { 0 };
            } else if self.auto_convert == 2 && !self.converting {
                self.auto_convert = 0;
                self.wait_frames = 0;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        } else if self.frames + 1 == 120 && !gpu_ready && self.wait_frames < 6000 {
            self.wait_frames += 1;
            std::thread::sleep(std::time::Duration::from_millis(5));
        } else {
            self.frames += 1;
        }
        if self.frames == 120 {
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(egui::UserData::default()));
        }
        let shot = ctx.input(|i| i.events.iter().find_map(|e| if let egui::Event::Screenshot { image, .. } = e { Some(image.clone()) } else { None }));
        if let Some(img) = shot {
            let [w, h] = img.size;
            let bytes: Vec<u8> = img.pixels.iter().flat_map(|c| c.to_array()).collect();
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Some(buf) = image::RgbaImage::from_raw(w as u32, h as u32, bytes) {
                if let Err(e) = buf.save(&path) {
                    eprintln!("could not save screenshot: {}", e);
                }
            }
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.frames > 900 {
            eprintln!("screenshot timed out");
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

fn channels_name(c: u32) -> String {
    match c {
        0 => "—".into(),
        1 => "Mono".into(),
        2 => "Stereo".into(),
        6 => "5.1".into(),
        8 => "7.1".into(),
        n => format!("{} ch", n),
    }
}

/// Source file vs. the file this profile will make, with the expected size.
fn compare_table(ui: &mut egui::Ui, p: &Profile, task: Option<&(String, PathBuf, MediaInfo, Edits, usize)>, hw: Option<String>) {
    ui.label(RichText::new("Original file → new file").strong().size(14.0));
    let Some((name, path, info, edits, track)) = task else {
        ui.add_space(8.0);
        ui.label(RichText::new("Add a file and select it to compare the original with the new file and see the expected size.").weak());
        return;
    };
    let weak = ui.visuals().weak_text_color();
    colored(ui, name, weak);
    ui.add_space(4.0);
    let src_audio = info.audio.get(*track).or_else(|| info.audio.first());
    let (w, h) = engine::out_dims(p, info, edits);
    let vk = engine::target_video_kbps(p, info, edits);
    let ak = engine::target_audio_kbps(p, info, *track);
    let est = engine::estimate_output(p, info, edits, *track);
    let dur = edits.effective_duration(info.duration);
    let video_out = info.has_video && p.vcodec != VCodec::None;
    let audio_out = src_audio.is_some() && p.acodec != ACodec::None;
    let dash = || "—".to_string();
    let fps_src = if info.fps > 0.0 { profiles::trim_float((info.fps * 100.0).round() / 100.0) } else { dash() };

    let mut rows: Vec<(&str, String, String)> = Vec::new();
    let src_ext = path.extension().map(|e| format!(".{}", e.to_string_lossy().to_lowercase())).unwrap_or_else(dash);
    rows.push(("File type", src_ext, format!(".{}", p.ext)));
    rows.push(("Duration", engine::fmt_time(info.duration), engine::fmt_time(dur)));
    if info.has_video {
        let vcodec = match p.vcodec {
            VCodec::None => "removed".to_string(),
            VCodec::Copy => format!("{} (kept)", info.vcodec),
            v => {
                let mut s = v.label().to_string();
                if let Some(h) = hw.as_ref().filter(|_| v.gpu_capable()) {
                    s.push_str(&format!(" ⚡ on {}", h));
                }
                s
            }
        };
        rows.push(("Video codec", info.vcodec.clone(), vcodec));
        rows.push(("Video size", format!("{}×{}", info.width, info.height), if video_out { format!("{}×{}", w, h) } else { dash() }));
        let fps_out = if !video_out {
            dash()
        } else if p.fps > 0.0 && p.vcodec != VCodec::Copy {
            profiles::trim_float(p.fps)
        } else {
            fps_src.clone()
        };
        rows.push(("Frame rate (fps)", fps_src.clone(), fps_out));
        let vb_src = info.video_bps();
        rows.push((
            "Video bitrate (kbps)",
            if vb_src > 0 { format!("{}", vb_src / 1000) } else { dash() },
            match vk {
                Some(k) if video_out => format!("≈ {:.0}", k),
                None => "varies (lossless)".into(),
                _ => dash(),
            },
        ));
    }
    if let Some(a) = src_audio {
        let acodec = match p.acodec {
            ACodec::None => "removed".to_string(),
            ACodec::Copy => format!("{} (kept)", a.codec),
            c => c.label().to_string(),
        };
        rows.push(("Audio codec", a.codec.clone(), acodec));
        let rate_out = if !audio_out {
            dash()
        } else if p.sample_rate > 0 && p.acodec != ACodec::Copy {
            p.sample_rate.to_string()
        } else {
            a.sample_rate.to_string()
        };
        rows.push(("Sample rate (Hz)", if a.sample_rate > 0 { a.sample_rate.to_string() } else { dash() }, rate_out));
        let ch_out = if !audio_out {
            dash()
        } else if p.channels > 0 && p.acodec != ACodec::Copy {
            channels_name(p.channels as u32)
        } else {
            channels_name(a.channels)
        };
        rows.push(("Channels", channels_name(a.channels), ch_out));
        rows.push(("Audio bitrate (kbps)", if a.bitrate > 0 { format!("{}", a.bitrate / 1000) } else { dash() }, if audio_out { format!("≈ {:.0}", ak) } else { dash() }));
    }
    rows.push(("File size", engine::fmt_bytes(info.size), est.map(|e| format!("≈ {}", engine::fmt_bytes(e))).unwrap_or_else(|| "can't estimate".into())));

    egui::Frame::new().fill(ui.visuals().extreme_bg_color).corner_radius(6).inner_margin(egui::Margin::same(8)).show(ui, |ui| {
        egui::Grid::new("compare").num_columns(3).striped(true).spacing([18.0, 6.0]).min_col_width(90.0).show(ui, |ui| {
            ui.label(RichText::new("").strong());
            ui.label(RichText::new("Original").strong());
            ui.label(RichText::new("New file").strong().color(ACCENT));
            ui.end_row();
            for (k, a, b) in rows {
                ui.label(RichText::new(k).weak());
                ui.label(&a);
                let norm = |s: &str| s.trim_start_matches("≈ ").to_lowercase().replace(" / avc", "").replace("h.264", "h264");
                let changed = norm(&a) != norm(&b) && !b.ends_with("(kept)") && !b.starts_with(&format!("{} ", a));
                ui.label(if changed { RichText::new(b).color(ACCENT) } else { RichText::new(b) });
                ui.end_row();
            }
        });
    });
    ui.add_space(8.0);
    if let Some(e) = est {
        let pct = if info.size > 0 { e as f64 / info.size as f64 * 100.0 } else { 0.0 };
        ui.label(RichText::new(format!("Expected size: {}", engine::fmt_bytes(e))).size(18.0).strong().color(GOOD));
        if pct > 0.0 {
            ui.label(RichText::new(format!("about {:.0}% of the original ({})", pct, engine::fmt_bytes(info.size))).weak());
        }
        ui.label(RichText::new("An estimate: the real size depends on how busy the picture is.").weak().size(11.0));
    } else {
        ui.label(RichText::new("Expected size: can't be estimated for lossless formats").weak());
    }
}

fn collect_dir(dir: &Path, out: &mut Vec<PathBuf>, depth: u32) {
    if depth > 4 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<PathBuf> = rd.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            collect_dir(&p, out, depth + 1);
        } else if is_media(&p) {
            out.push(p);
        }
    }
}

/// Main titles of an unprotected DVD (VIDEO_TS) or Blu-ray (BDMV) folder.
fn disc_titles(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let vts = if dir.join("VIDEO_TS").is_dir() { dir.join("VIDEO_TS") } else { dir.to_path_buf() };
    if let Ok(rd) = std::fs::read_dir(&vts) {
        let mut v: Vec<PathBuf> = rd
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                let n = p.file_name().map(|s| s.to_string_lossy().to_uppercase()).unwrap_or_default();
                n.starts_with("VTS_") && n.ends_with(".VOB") && !n.ends_with("_0.VOB")
            })
            .collect();
        v.sort();
        out.extend(v);
    }
    let stream = [dir.join("BDMV").join("STREAM"), dir.join("STREAM")].into_iter().find(|p| p.is_dir());
    if let Some(s) = stream {
        if let Ok(rd) = std::fs::read_dir(&s) {
            let mut v: Vec<(u64, PathBuf)> = rd
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|p| p.extension().map(|e| e.to_string_lossy().eq_ignore_ascii_case("m2ts")).unwrap_or(false))
                .map(|p| (std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0), p))
                .collect();
            v.sort_by_key(|a| std::cmp::Reverse(a.0));
            out.extend(v.into_iter().filter(|(sz, _)| *sz > 50 * 1024 * 1024).map(|(_, p)| p));
        }
    }
    out
}

impl App {
    /// One frame of the whole window.
    fn ui(&mut self, ctx: &egui::Context) {
        self.poll();
        let dropped: Vec<PathBuf> = ctx.input(|i| i.raw.dropped_files.iter().filter_map(|f| f.path.clone()).collect());
        if !dropped.is_empty() {
            self.add_paths(dropped);
        }
        self.shortcuts(ctx);
        if self.selected.is_some() && self.preview_key.is_none() {
            self.request_preview();
        }

        egui::TopBottomPanel::top("toolbar").frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(egui::Margin::symmetric(8, 8))).show(ctx, |ui| self.toolbar(ui));
        egui::TopBottomPanel::bottom("output").frame(egui::Frame::side_top_panel(&ctx.style()).inner_margin(egui::Margin::symmetric(10, 4))).show(ctx, |ui| self.bottom_bar(ui));
        egui::SidePanel::right("preview").resizable(true).default_width(360.0).min_width(280.0).show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| self.preview_panel(ui));
        });
        egui::CentralPanel::default().show(ctx, |ui| self.task_list(ui));

        if self.show_picker {
            self.picker_window(ctx);
        }
        if self.show_customize {
            self.customize_window(ctx);
        }
        if self.show_options {
            self.options_window(ctx);
        }
        self.edit_window(ctx);
        self.log_window(ctx);
        if self.show_about {
            self.about_window(ctx);
        }
        if self.converting || self.preview_busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }
        // Progress in the window title, so it shows on the taskbar too.
        let pct = if self.converting { Some((self.overall * 100.0) as u32) } else { None };
        if pct != self.title_pct {
            self.title_pct = pct;
            let title = match pct {
                Some(p) => format!("{}% – ReelForge", p),
                None => TITLE.to_string(),
            };
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
        }
        self.screenshot_tick(ctx);
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.ui(ctx);
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        for t in &self.tasks {
            if let Some(j) = &t.job {
                j.cancel();
            }
        }
        if let Some((j, _, _)) = &self.merge_job {
            j.cancel();
        }
        self.save_settings();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pick(stem: &str, source: &str, policy: ExistsPolicy, on_disk: &[&str], in_use: &[&str]) -> Option<String> {
        let dir = Path::new("out");
        let has = |list: &[&str], o: &Path| list.iter().any(|n| same_path(&dir.join(n), o));
        choose_output(dir, stem, "mp4", Path::new(source), policy, &|o| has(on_disk, o), &|o| has(in_use, o)).map(|p| file_name(&p))
    }

    #[test]
    fn a_free_name_is_used_as_is() {
        assert_eq!(pick("clip", "in/clip.mov", ExistsPolicy::Rename, &[], &[]).as_deref(), Some("clip.mp4"));
    }

    #[test]
    fn an_existing_file_follows_the_chosen_rule() {
        assert_eq!(pick("clip", "in/clip.mov", ExistsPolicy::Rename, &["clip.mp4", "clip (1).mp4"], &[]).as_deref(), Some("clip (2).mp4"));
        assert_eq!(pick("clip", "in/clip.mov", ExistsPolicy::Overwrite, &["clip.mp4"], &[]).as_deref(), Some("clip.mp4"));
        assert_eq!(pick("clip", "in/clip.mov", ExistsPolicy::Skip, &["clip.mp4"], &[]), None);
    }

    #[test]
    fn the_original_is_never_written_over() {
        // Same folder, same name, and Windows does not care about upper/lower case.
        for policy in [ExistsPolicy::Rename, ExistsPolicy::Overwrite, ExistsPolicy::Skip] {
            assert_eq!(pick("CLIP", "out/clip.MP4", policy, &["clip.MP4"], &[]).as_deref(), Some("CLIP (1).mp4"), "{policy:?}");
        }
        assert!(same_path(Path::new("C:/Videos/A.MP4"), Path::new("c:\\videos\\a.mp4")));
        assert!(!same_path(Path::new("C:/Videos/a.mp4"), Path::new("C:/Videos/b.mp4")));
    }

    #[test]
    fn two_files_with_one_name_get_two_outputs() {
        // "a/clip.mov" is already being converted to out/clip.mp4, which is not
        // on disk yet: "b/clip.avi" must not pick the same name, whatever the rule.
        for policy in [ExistsPolicy::Rename, ExistsPolicy::Overwrite, ExistsPolicy::Skip] {
            assert_eq!(pick("clip", "b/clip.avi", policy, &[], &["clip.mp4"]).as_deref(), Some("clip (1).mp4"), "{policy:?}");
        }
        assert_eq!(pick("clip", "c/clip.mkv", ExistsPolicy::Overwrite, &["clip.mp4"], &["CLIP.mp4", "clip (1).mp4"]).as_deref(), Some("clip (2).mp4"));
    }

    #[test]
    fn convert_carries_on_where_it_stopped() {
        // Two done, one failed or stopped, one new: only the last two run.
        assert_eq!(pick_targets(&[(1, true), (2, true), (3, false), (4, false)], false), vec![3, 4]);
        // Everything done: converting again is what was asked for.
        assert_eq!(pick_targets(&[(1, true), (2, true)], false), vec![1, 2]);
        // A merge always needs every part.
        assert_eq!(pick_targets(&[(1, true), (2, false)], true), vec![1, 2]);
        assert_eq!(pick_targets(&[(7, false)], true), vec![7]);
        assert!(pick_targets(&[], false).is_empty());
    }

    #[test]
    fn a_dropped_subtitle_finds_its_video() {
        let videos = vec![(1, "holiday".to_string()), (2, "holiday part 2".to_string()), (3, "wedding".to_string())];
        assert_eq!(subtitle_target(&videos, "Wedding", Some(1)), Some(3));
        assert_eq!(subtitle_target(&videos, "holiday.en", Some(3)), Some(1));
        assert_eq!(subtitle_target(&videos, "holiday part 2.ku", None), Some(2));
        // No name match: the selected video, if it can take subtitles.
        assert_eq!(subtitle_target(&videos, "subs", Some(2)), Some(2));
        assert_eq!(subtitle_target(&videos, "subs", Some(99)), None);
        assert_eq!(subtitle_target(&videos, "subs", None), None);
        // A single video needs no selecting.
        assert_eq!(subtitle_target(&videos[..1], "subs", None), Some(1));
        assert_eq!(subtitle_target(&[], "subs", Some(1)), None);
        // Kurdish names match too.
        let ku = vec![(5, "گۆرانی".to_string()), (6, "فیلم".to_string())];
        assert_eq!(subtitle_target(&ku, "فیلم", Some(5)), Some(6));
    }

    #[test]
    fn long_files_weigh_more_in_the_overall_progress() {
        assert_eq!(overall_progress(&[]), 0.0);
        assert_eq!(overall_progress(&[(60.0, 1.0), (60.0, 0.0)]), 0.5);
        let p = overall_progress(&[(3600.0, 0.5), (60.0, 1.0)]);
        assert!((p - 1860.0 / 3660.0).abs() < 1e-4, "{p}");
        // Unknown lengths still count as something.
        assert_eq!(overall_progress(&[(0.0, 1.0), (0.0, 0.0)]), 0.5);
        assert_eq!(overall_progress(&[(10.0, 7.0)]), 1.0);
    }

    #[test]
    fn time_left_waits_until_it_can_tell() {
        assert_eq!(time_left(1.0, 0.5), None);
        assert_eq!(time_left(30.0, 0.01), None);
        assert_eq!(time_left(30.0, 1.0), None);
        assert_eq!(time_left(30.0, 0.25), Some(90.0));
        assert_eq!(time_left(60.0, 0.5), Some(60.0));
    }

    #[test]
    fn the_summary_says_what_happened() {
        let base = BatchStats { done: 3, secs: 80.0, ..Default::default() };
        assert_eq!(summary_text(&base), "Finished: 3 converted in 01:20");
        let sized = BatchStats { in_bytes: 1000 * 1024 * 1024, out_bytes: 400 * 1024 * 1024, gpu: 2, ..base.clone() };
        assert_eq!(summary_text(&sized), "Finished: 3 converted in 01:20 · 1000.0 MB → 400.0 MB (60% smaller) · 2 with the graphics card");
        let bigger = BatchStats { in_bytes: 100 * 1024, out_bytes: 150 * 1024, ..base.clone() };
        assert!(summary_text(&bigger).ends_with("(50% larger)"));
        let same = BatchStats { in_bytes: 5000, out_bytes: 5001, ..base.clone() };
        assert!(summary_text(&same).ends_with("(same size)"));
        let mixed = BatchStats { failed: 1, skipped: 2, cancelled: 1, ..base.clone() };
        assert_eq!(summary_text(&mixed), "Finished: 3 converted in 01:20 · 2 skipped (file already there) · 1 cancelled · 1 failed (click the status to see why)");
        let stopped = BatchStats { done: 1, cancelled: 2, stopped: true, ..Default::default() };
        assert_eq!(summary_text(&stopped), "Stopped: 1 converted");
    }

    #[test]
    fn media_and_subtitle_files_are_told_apart() {
        assert!(is_media(Path::new("a/Film.MKV")));
        assert!(is_media(Path::new("song.flac")));
        assert!(!is_media(Path::new("notes.txt")));
        assert!(!is_media(Path::new("noextension")));
        assert!(is_subtitle(Path::new("Film.SRT")));
        assert!(is_subtitle(Path::new("film.en.ass")));
        assert!(!is_subtitle(Path::new("film.mkv")));
        assert_eq!(file_name(Path::new("a/b/ڕێگا.mp4")), "ڕێگا.mp4");
    }

    #[test]
    fn folders_and_discs_are_scanned() {
        let dir = std::env::temp_dir().join(format!("reelforge_app_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let touch = |rel: &str| {
            let p = dir.join(rel);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(&p, b"x").unwrap();
        };
        for f in ["b.mp4", "a.mkv", "notes.txt", "sub/c.mp3", "sub/deeper/d.wav", "sub/cover.jpg"] {
            touch(f);
        }
        let mut found = Vec::new();
        collect_dir(&dir, &mut found, 0);
        let names: Vec<String> = found.iter().map(|p| file_name(p)).collect();
        assert_eq!(names, ["a.mkv", "b.mp4", "c.mp3", "d.wav"]);

        // A DVD folder: the menu file (…_0.VOB) is left out.
        for f in ["disc/VIDEO_TS/VTS_01_0.VOB", "disc/VIDEO_TS/VTS_01_2.VOB", "disc/VIDEO_TS/VTS_01_1.VOB", "disc/VIDEO_TS/VIDEO_TS.IFO"] {
            touch(f);
        }
        let titles: Vec<String> = disc_titles(&dir.join("disc")).iter().map(|p| file_name(p)).collect();
        assert_eq!(titles, ["VTS_01_1.VOB", "VTS_01_2.VOB"]);
        assert!(disc_titles(&dir.join("sub")).is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    // ------------------------------------------------------------ the window, without a screen

    fn video() -> MediaInfo {
        MediaInfo {
            ok: true,
            duration: 60.0,
            width: 1920,
            height: 1080,
            fps: 30.0,
            vcodec: "h264".into(),
            acodec: "aac".into(),
            has_video: true,
            audio: vec![engine::Track { codec: "aac".into(), lang: "kur".into(), title: "دەنگی ڕەسەن".into(), channels: 2, sample_rate: 48000, bitrate: 128_000, ..Default::default() }],
            subs: vec![
                engine::Track { codec: "subrip".into(), title: "کوردی ڕێنووس".into(), text_based: true, ..Default::default() },
                engine::Track { codec: "hdmv_pgs_subtitle".into(), ..Default::default() },
            ],
            size: 50_000_000,
            bitrate: 6_000_000,
            ..Default::default()
        }
    }

    /// An app that starts no FFmpeg and saves nothing, with these files loaded.
    fn test_app(paths: &[&str]) -> (egui::Context, App) {
        let ctx = egui::Context::default();
        let mut app = App::build(&ctx, None, false);
        assert!(!app.persist && !app.live);
        for p in paths {
            let id = app.push_task(PathBuf::from(p));
            app.task_mut(id).unwrap().info = Some(video());
        }
        (ctx, app)
    }

    fn frame(ctx: &egui::Context, app: &mut App, events: Vec<egui::Event>) {
        let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 800.0))), events, ..Default::default() };
        let _ = ctx.run(input, |ctx| app.ui(ctx));
    }

    fn key(key: egui::Key) -> egui::Event {
        egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE }
    }

    fn ctrl(key: egui::Key) -> egui::Event {
        egui::Event::Key { key, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::CTRL | egui::Modifiers::COMMAND }
    }

    #[test]
    fn the_keyboard_works_the_file_list() {
        let (ctx, mut app) = test_app(&["C:/v/one.mp4", "C:/v/two.mp4", "C:/v/three.mp4"]);
        let ids: Vec<u64> = app.tasks.iter().map(|t| t.id).collect();
        frame(&ctx, &mut app, vec![]);
        assert_eq!(app.selected, Some(ids[0]), "the first file is selected when added");

        frame(&ctx, &mut app, vec![key(egui::Key::ArrowDown)]);
        assert_eq!(app.selected, Some(ids[1]));
        frame(&ctx, &mut app, vec![key(egui::Key::ArrowDown)]);
        frame(&ctx, &mut app, vec![key(egui::Key::ArrowDown)]);
        assert_eq!(app.selected, Some(ids[2]), "stops at the last file");
        frame(&ctx, &mut app, vec![key(egui::Key::ArrowUp)]);
        assert_eq!(app.selected, Some(ids[1]));

        for t in app.tasks.iter_mut() {
            t.checked = false;
        }
        frame(&ctx, &mut app, vec![ctrl(egui::Key::A)]);
        assert!(app.tasks.iter().all(|t| t.checked));

        // Enter opens the editor; while it is open the list keys do nothing.
        frame(&ctx, &mut app, vec![key(egui::Key::Enter)]);
        assert_eq!(app.edit_task, Some(ids[1]));
        frame(&ctx, &mut app, vec![key(egui::Key::Delete)]);
        assert_eq!(app.tasks.len(), 3);
        frame(&ctx, &mut app, vec![key(egui::Key::Escape)]);
        assert_eq!(app.edit_task, None);

        frame(&ctx, &mut app, vec![key(egui::Key::Delete)]);
        assert_eq!(app.tasks.iter().map(|t| t.id).collect::<Vec<_>>(), vec![ids[0], ids[2]]);
        assert_eq!(app.selected, Some(ids[2]), "the next file takes the selection");

        // A file that is being converted stays on the list.
        app.tasks[1].status = Status::Running;
        frame(&ctx, &mut app, vec![key(egui::Key::Delete)]);
        assert_eq!(app.tasks.len(), 2);
    }

    #[test]
    fn escape_closes_one_window_at_a_time() {
        let (ctx, mut app) = test_app(&["C:/v/one.mp4"]);
        app.show_picker = true;
        app.show_options = true;
        app.show_about = true;
        frame(&ctx, &mut app, vec![]);
        frame(&ctx, &mut app, vec![key(egui::Key::Escape)]);
        assert!(!app.show_about && app.show_options && app.show_picker);
        frame(&ctx, &mut app, vec![key(egui::Key::Escape)]);
        assert!(!app.show_options && app.show_picker);
        frame(&ctx, &mut app, vec![key(egui::Key::Escape)]);
        assert!(!app.show_picker);
    }

    /// Draws every view in every state, with Kurdish names, in both looks:
    /// nothing may panic and the right-to-left text path must be exercised.
    #[test]
    fn every_view_draws() {
        let kurdish = "C:/v/فیلمی ڵاوێن ـ بەڵگەنامە.mkv";
        let (ctx, mut app) = test_app(&[kurdish, "C:/v/two.mp4", "C:/v/three.mp4", "C:/v/four.mp4", "C:/v/five.mp4", "C:/v/six.mp4", "C:/v/song.mp3"]);
        let ids: Vec<u64> = app.tasks.iter().map(|t| t.id).collect();
        let result = |progress: f32| JobState {
            progress,
            speed: 2.5,
            eta_secs: 30.0,
            out_bytes: 20_000_000,
            engine: "GPU: NVIDIA".into(),
            elapsed: 12.0,
            log: vec!["ffmpeg -i x".into(), format!("Output: {kurdish}")],
            error: "boom".into(),
            ..Default::default()
        };
        let states = [Status::Done, Status::Running, Status::Failed, Status::Skipped, Status::Cancelled, Status::Ready];
        for (i, st) in states.iter().enumerate() {
            app.tasks[i].status = *st;
            app.tasks[i].result = Some(result(0.4));
            app.tasks[i].output = Some(PathBuf::from("C:/out/x.mp4"));
        }
        app.tasks[0].subtitle = SubChoice::External(PathBuf::from("C:/v/ژێرنووس.srt"));
        app.tasks[0].edits.watermark = "ڕێگای ئاشتی".into();
        app.tasks[0].split_minutes = 5.0;
        app.tasks[6].info = Some(MediaInfo { has_video: false, subs: vec![], ..video() });
        app.batch = ids.clone();
        app.converting = true;
        app.started_at = Some(std::time::Instant::now());
        app.settings.output_dir = PathBuf::from("D:/ڤیدیۆکان/گۆڕدراو");
        for dark in [true, false] {
            app.settings.dark = dark;
            app.apply_theme(&ctx);
            frame(&ctx, &mut app, vec![]);
            assert!(app.converting, "a running file keeps the conversion going");
            assert!(app.overall > 0.0 && app.overall < 1.0);

            app.show_picker = true;
            for (cat, search) in [("Recommended", ""), ("Recent", ""), ("Favorites", ""), ("Common Video", "hevc 4k")] {
                app.picker_cat = cat.into();
                app.picker_search = search.into();
                frame(&ctx, &mut app, vec![]);
            }
            app.show_picker = false;

            app.show_customize = true;
            frame(&ctx, &mut app, vec![]);
            app.selected = Some(ids[6]);
            frame(&ctx, &mut app, vec![]);
            app.selected = Some(ids[0]);
            app.show_customize = false;

            app.show_options = true;
            for tab in [OptTab::General, OptTab::Conversion, OptTab::Acceleration, OptTab::Snapshot] {
                app.opt_tab = tab;
                frame(&ctx, &mut app, vec![]);
            }
            app.show_options = false;

            for id in [ids[0], ids[6]] {
                app.edit_task = Some(id);
                for tab in [EditTab::Trim, EditTab::Crop, EditTab::Effects, EditTab::Watermark, EditTab::Audio, EditTab::Subtitle] {
                    app.edit_tab = tab;
                    frame(&ctx, &mut app, vec![]);
                }
            }
            app.edit_task = None;

            app.log_task = Some(ids[0]);
            frame(&ctx, &mut app, vec![]);
            app.log_task = None;
            app.show_about = true;
            frame(&ctx, &mut app, vec![]);
            app.show_about = false;
        }
        // The Kurdish name went through Windows' own text drawing.
        #[cfg(windows)]
        assert!(bidi::cached_lines() > 0);

        // When the last file ends, the summary appears and nothing else happens.
        app.tasks[1].status = Status::Done;
        frame(&ctx, &mut app, vec![]);
        assert!(!app.converting);
        let (msg, _) = app.message.clone().unwrap();
        assert!(msg.starts_with("Finished: 2 converted"), "{msg}");
        assert!(msg.contains("1 failed") && msg.contains("1 skipped") && msg.contains("1 cancelled"), "{msg}");
        assert!(!app.shutdown_pending);
        frame(&ctx, &mut app, vec![]);

        // An empty list draws the welcome view.
        app.tasks.clear();
        app.selected = None;
        frame(&ctx, &mut app, vec![]);
    }

    #[test]
    fn stop_never_shuts_down_or_reports_success() {
        let (ctx, mut app) = test_app(&["C:/v/one.mp4", "C:/v/two.mp4", "C:/v/three.mp4"]);
        app.batch = app.tasks.iter().map(|t| t.id).collect();
        app.tasks[0].status = Status::Done;
        app.tasks[0].result = Some(JobState { out_bytes: 10_000_000, ..Default::default() });
        app.tasks[1].status = Status::Running;
        app.tasks[2].status = Status::Queued;
        app.converting = true;
        app.shutdown_after = true;
        app.settings.open_folder_after = true;
        app.started_at = Some(std::time::Instant::now());
        app.stop_all();
        assert_eq!(app.tasks[2].status, Status::Ready, "waiting files go back to ready");
        // The running file ends as cancelled a moment later.
        app.tasks[1].status = Status::Cancelled;
        frame(&ctx, &mut app, vec![]);
        assert!(!app.converting);
        assert!(!app.shutdown_pending, "Stop must never lead to a shutdown");
        let (msg, _) = app.message.clone().unwrap();
        assert!(msg.starts_with("Stopped: 1 converted"), "{msg}");
        // Convert now takes only what is left.
        assert_eq!(app.convert_targets(), vec![app.tasks[1].id, app.tasks[2].id]);
    }

    #[test]
    fn same_named_files_never_share_an_output() {
        let (_ctx, mut app) = test_app(&["C:/a/clip.mov", "C:/b/clip.avi", "C:/c/CLIP.mkv"]);
        let dir = std::env::temp_dir().join(format!("reelforge_out_test_{}", std::process::id()));
        app.settings.output_dir = dir.clone();
        app.settings.exists = ExistsPolicy::Overwrite;
        let profile = app.current.clone();
        let mut outs = Vec::new();
        for i in 0..3 {
            let out = app.output_for(&app.tasks[i], &profile).expect("an output");
            app.batch_outputs.push(out.clone());
            outs.push(file_name(&out));
        }
        assert_eq!(outs, ["clip.mp4", "clip (1).mp4", "CLIP (2).mp4"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_dropped_subtitle_file_is_attached() {
        let dir = std::env::temp_dir().join(format!("reelforge_sub_test_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let srt = dir.join("Two.ku.srt");
        std::fs::write(&srt, "1\n00:00:01,000 --> 00:00:02,000\nسڵاو\n").unwrap();
        let (ctx, mut app) = test_app(&["C:/v/one.mp4", "C:/v/two.mp4"]);
        app.add_paths(vec![srt.clone()]);
        assert_eq!(app.tasks.len(), 2, "a subtitle file is not a file to convert");
        assert_eq!(app.tasks[0].subtitle, SubChoice::None);
        assert_eq!(app.tasks[1].subtitle, SubChoice::External(srt.clone()));
        assert!(app.message.as_ref().unwrap().0.contains("two.mp4"));
        frame(&ctx, &mut app, vec![]);

        // With no video to give it to, it says what to do.
        let (_ctx2, mut empty) = test_app(&[]);
        empty.add_paths(vec![srt]);
        assert!(empty.tasks.is_empty());
        assert!(empty.message.as_ref().unwrap().0.starts_with("Select the video first"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_summary_compares_sizes_of_whole_files_only() {
        let (_ctx, mut app) = test_app(&["C:/v/one.mp4", "C:/v/two.mp4", "C:/v/three.mp4", "C:/v/old.mp4"]);
        let done = |bytes: u64| Some(JobState { out_bytes: bytes, engine: "GPU: Intel".into(), ..Default::default() });
        for t in app.tasks.iter_mut() {
            t.status = Status::Done;
            t.result = done(10_000_000);
        }
        app.tasks[1].edits.trim_end = 20.0; // trimmed: its size says nothing
        app.batch = app.tasks[..3].iter().map(|t| t.id).collect(); // "old" was converted earlier
        let s = app.batch_stats();
        assert_eq!((s.done, s.gpu, s.failed), (3, 3, 0));
        assert_eq!((s.in_bytes, s.out_bytes), (100_000_000, 20_000_000));

        // A merge makes one file out of all of them.
        app.batch_merged = true;
        app.tasks[1].edits.trim_end = 0.0;
        let m = app.batch_stats();
        assert_eq!(m.done, 1);
        assert_eq!((m.in_bytes, m.out_bytes), (150_000_000, 10_000_000));
    }
}
