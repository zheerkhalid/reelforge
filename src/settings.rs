use crate::gpu::AccelChoice;
use crate::profiles::Profile;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum ExistsPolicy {
    Rename,
    Overwrite,
    Skip,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub output_dir: PathBuf,
    pub profile_id: u32,
    pub recent: Vec<u32>,
    pub favorites: Vec<u32>,
    pub custom_profiles: Vec<Profile>,
    /// "Enable GPU Encode Acceleration".
    pub gpu_encode: bool,
    /// "Enable GPU Decode Acceleration".
    pub gpu_decode: bool,
    pub accel: AccelChoice,
    /// "Maximum Simultaneous Running Processes".
    pub max_parallel: u32,
    /// 0 = all cores.
    pub threads: u32,
    pub exists: ExistsPolicy,
    pub snapshot_dir: PathBuf,
    pub snapshot_jpg: bool,
    pub snapshot_deinterlace: bool,
    pub open_folder_after: bool,
    pub same_folder_as_source: bool,
    pub dark: bool,
}

impl Default for Settings {
    fn default() -> Self {
        let videos = dirs::video_dir().or_else(dirs::home_dir).unwrap_or_else(|| PathBuf::from("."));
        let pics = dirs::picture_dir().unwrap_or_else(|| videos.clone());
        Settings {
            output_dir: videos.join("Converted"),
            profile_id: 0,
            recent: vec![],
            favorites: vec![],
            custom_profiles: vec![],
            gpu_encode: true,
            gpu_decode: true,
            accel: AccelChoice::Auto,
            max_parallel: 2,
            threads: 0,
            exists: ExistsPolicy::Rename,
            snapshot_dir: pics.join("Snapshots"),
            snapshot_jpg: false,
            snapshot_deinterlace: false,
            open_folder_after: true,
            same_folder_as_source: false,
            dark: true,
        }
    }
}

fn path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("ReelForge").join("settings.json"))
}

impl Settings {
    /// Neutral settings for documentation pictures (`--screenshot`): nothing
    /// of the person's own setup shows, and nothing is saved.
    pub fn for_screenshot() -> Self {
        Settings { output_dir: PathBuf::from(r"D:\Videos\Converted"), open_folder_after: false, ..Default::default() }
    }
    pub fn load() -> Self {
        path().and_then(|p| std::fs::read(p).ok()).and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default()
    }
    pub fn save(&self) {
        if let Some(p) = path() {
            if let Some(d) = p.parent() {
                let _ = std::fs::create_dir_all(d);
            }
            if let Ok(s) = serde_json::to_vec_pretty(self) {
                let _ = std::fs::write(p, s);
            }
        }
    }
    /// Keeps the 20 most recent profiles.
    pub fn push_recent(&mut self, id: u32) {
        self.recent.retain(|x| *x != id);
        self.recent.insert(0, id);
        self.recent.truncate(20);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_profiles_are_unique_newest_first_and_capped() {
        let mut s = Settings::default();
        for id in 0..30 {
            s.push_recent(id);
        }
        s.push_recent(25);
        assert_eq!(s.recent.len(), 20);
        assert_eq!(s.recent[0], 25);
        assert_eq!(s.recent.iter().filter(|x| **x == 25).count(), 1);
    }

    #[test]
    fn settings_from_an_older_version_still_load() {
        // Fields added later fall back to their defaults.
        let s: Settings = serde_json::from_str(r#"{"max_parallel": 3, "dark": false}"#).unwrap();
        assert_eq!(s.max_parallel, 3);
        assert!(!s.dark);
        assert!(s.gpu_encode);
        assert_eq!(s.exists, ExistsPolicy::Rename);
    }

    #[test]
    fn screenshot_settings_show_no_personal_folder() {
        let s = Settings::for_screenshot();
        assert_eq!(s.output_dir, PathBuf::from(r"D:\Videos\Converted"));
    }
}
