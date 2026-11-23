# Changelog

All notable changes to ReelForge are listed here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the version numbers follow [Semantic Versioning](https://semver.org/).

## [1.1.0] - 2026-11-24

### Added
- **Drop a subtitle file on the window** (`.srt`, `.ass`, `.ssa`, `.vtt`) and it is burned into the video with the same name, or into the selected video. Subtitle files can also be picked together with the videos in *Add files*.
- **Size saved in the finish line**: "Finished: 3 converted in 01:20 · 1.2 GB → 410 MB (66% smaller)".
- **Time left** for the whole conversion next to the progress bar, and the **progress in the window title**, so it shows on the taskbar while ReelForge is in the background.
- **Right-click a file** for a menu: Edit, Play the original, Show the original in its folder, Show the converted file, Conversion log, Remove from the list. **Double-click** a file to edit it.
- **Keyboard shortcuts**: `Ctrl+O` add files, `↑` `↓` select, `Enter` edit, `Delete` remove, `Ctrl+A` tick all, `Esc` close the open window.
- **Clear done** button: takes the converted files off the list and keeps the rest.
- **Cancel shutdown** button after a conversion that ends with *Shut down when done*.
- **Type a time** in the trim boxes: `1:30`, `0:01:30.5` or `90`. Times that are not whole seconds now show their tenths.
- The file list shows which files have subtitles switched on.

### Changed
- **Kurdish and Arabic text is now drawn by Windows itself**, the way File Explorer draws it. File names, track titles, subtitle names, folders, messages and the conversion log are joined and ordered correctly, including the Kurdish letters ڕ ڵ ە ێ, which used to stand apart.
- **Convert carries on where it stopped.** After a stop or a failed file, pressing Convert again only does the files that are not done yet. When every ticked file is already done, it converts them all again as before.
- Files are read four at a time when you add them, so adding a folder with hundreds of files no longer starts hundreds of background programs at once.
- Long file names are cut with "…" instead of running into the status on the right.
- The overall progress bar counts only the files of the conversion that is running, weighed by their length after trimming.
- *Clear* is greyed out while converting instead of silently doing nothing.
- The About window shows the real version number and lists the shortcuts.
- `scripts\package.ps1` builds into `CARGO_TARGET_DIR` when it is set.

### Fixed
- **Two files with the same name** (from different folders) converted at the same time were written to the same output file. Each now gets its own name.
- **The original could be overwritten** when "Next to the original" and "Overwrite" were both on and the name differed only in upper/lower case (for example `CLIP.MP4` → `clip.mp4`). The original is never written over now.
- **Stop no longer shuts the computer down.** With *Shut down when done* ticked, pressing Stop after at least one file had finished still turned the computer off, and still opened the output folder.
- **No more broken half-files.** A stopped or failed conversion left an unplayable partial file behind. It is now removed (a file that was there before the conversion is never deleted).
- With "Next to the original", the folder opened at the end is the one the files really went to, a merged file is saved next to the first original, and the unused output folder is no longer created.
- The finish line counted files converted in earlier runs as well.

## [1.0.1] - 2026-11-15

### Added
- Installer (`ReelForge-Setup.exe`) and portable ZIP.

### Changed
- The HEVC (H.265) choices in *Recommended* are labelled clearly: smaller file at the same quality, or very small file.

## [1.0.0] - 2026-11-15

### Added
- First release: video and audio converter with 409 ready-made profiles in 20 categories.
- Graphics-card acceleration (NVIDIA NVENC, AMD AMF, Intel Quick Sync), each encoder tested before it is offered, with automatic fallback to the processor.
- Trim, crop, rotate, colour adjustments, text watermark, volume and loudness normalising.
- Subtitles burned in from tracks inside the file or from subtitle files.
- Merge, split, remux, snapshots, parallel conversions, expected output size, custom profiles.

[1.1.0]: https://github.com/zheerkhalid/reelforge/releases/tag/v1.1.0
[1.0.1]: https://github.com/zheerkhalid/reelforge/releases/tag/v1.0.1
[1.0.0]: https://github.com/zheerkhalid/reelforge/releases/tag/v1.0.0
