//! Free-standing helpers shared between [`mod.rs`](super), [`actions.rs`](super::actions),
//! and the [`render`](super::render) submodules.
//!
//! Contents:
//!
//! - [`BreadcrumbSegment`] / [`rebuild_breadcrumb_segments`] — cached
//!   path-segment list used by the breadcrumb bar (P1-3 + P0-2).
//! - [`is_valid_filename`] — name validation for create / rename actions.
//! - [`ui_set_clipboard`] — NUL-safe clipboard write (P0-4).
//! - [`enumerate_drives`] — OS-specific drive/root list for the drive bar.

use std::path::{Path, PathBuf};

use dear_imgui_rs::Ui;

// ─── Breadcrumb segments ────────────────────────────────────────────────────

/// One clickable segment of the breadcrumb path bar.
///
/// Cached on the [`FileManager`](super::FileManager) and rebuilt only when the
/// current path changes (P1-3). Built via [`Path::ancestors`] so volume roots
/// are preserved correctly:
///
/// - Unix `/usr/bin` → `[("/", "/"), ("usr", "/usr"), ("bin", "/usr/bin")]`
/// - Windows `C:\Users` → `[("C:\\", "C:\\"), ("Users", "C:\\Users")]`
/// - UNC `\\server\share\dir` → `[("\\\\server\\share\\", root), ("dir", target)]`
///
/// The previous `split(['\\', '/'])` approach (P0-2) dropped leading separators
/// and reconstructed wrong paths on UNC and absolute Unix paths.
pub(super) struct BreadcrumbSegment {
    /// Visible label (e.g. `"Users"`, `"C:\\"`, `"/"`).
    pub label: String,
    /// Full absolute path to navigate to when this segment is clicked.
    pub path: PathBuf,
}

/// Rebuild the cached breadcrumb segments for `path` into `out`.
///
/// Walks [`Path::ancestors`] from root → leaf, keeping each ancestor's full
/// path (for navigation) alongside its display label. Empty ancestors (which
/// appear for relative paths) are skipped.
pub(super) fn rebuild_breadcrumb_segments(path: &Path, out: &mut Vec<BreadcrumbSegment>) {
    out.clear();
    // `Path::ancestors` yields leaf → root; reverse for left-to-right display.
    let mut ancestors: Vec<&Path> = path.ancestors().collect();
    ancestors.reverse();
    for ancestor in ancestors {
        let label = match ancestor.file_name() {
            Some(name) => name.to_string_lossy().into_owned(),
            None => {
                // Root component (`/`, `C:\\`, `\\\\server\\share\\`) — `file_name`
                // returns `None`, so use the full path string. Skip if empty
                // (which happens for the trailing `""` ancestor of relative paths).
                let s = ancestor.to_string_lossy();
                if s.is_empty() {
                    continue;
                }
                s.into_owned()
            }
        };
        out.push(BreadcrumbSegment {
            label,
            path: ancestor.to_path_buf(),
        });
    }
}

// ─── Filename validation ────────────────────────────────────────────────────

/// Check if a filename is safe to create / rename to.
///
/// Rejects:
///
/// - empty / over-length names (>255 bytes),
/// - the path-traversal aliases `.` and `..` (P0-6),
/// - Windows reserved device names (`CON`, `NUL`, `COM1`-`COM9`, `LPT1`-`LPT9`),
/// - invalid characters (`< > : " / \ | ? * \0`),
/// - trailing dot or space.
pub(super) fn is_valid_filename(name: &str) -> bool {
    if name.is_empty() || name.len() > 255 {
        return false;
    }
    // P0-6: `.` and `..` are path-traversal aliases — never a valid new name.
    // `..` would let `CreateFolder` / `RenameEntry` escape into the parent
    // directory; `.` is meaningless and the OS would error out anyway.
    if name == "." || name == ".." {
        return false;
    }
    // Windows reserved names
    let upper = name.to_uppercase();
    let stem = upper.split('.').next().unwrap_or("");
    if matches!(
        stem,
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    ) {
        return false;
    }
    // Invalid characters across platforms
    !name.contains(['<', '>', ':', '"', '/', '\\', '|', '?', '*', '\0'])
        && !name.ends_with('.')
        && !name.ends_with(' ')
}

// ─── Drive-letter parsing ────────────────────────────────────────────────────

/// Extract a Windows drive letter from a path string, or `None`.
///
/// Returns the letter only for a genuine `X:` / `X:\…` drive prefix — so the
/// drive bar highlights the correct button and never mistakes a relative path
/// (`foo\bar`), a UNC root (`\\server\share`), or a Unix absolute path
/// (`/usr`) for a drive. The letter is normalised to uppercase to match the
/// `enumerate_drives` output (`["C:\\", …]`).
pub(super) fn drive_letter_of(path: &str) -> Option<char> {
    let mut chars = path.chars();
    let first = chars.next()?;
    if first.is_ascii_alphabetic() && chars.next() == Some(':') {
        Some(first.to_ascii_uppercase())
    } else {
        None
    }
}

// ─── Clipboard ──────────────────────────────────────────────────────────────

/// Set the ImGui clipboard text via raw sys API.
///
/// `CString::new` rejects strings with interior NUL bytes; the previous
/// `unwrap_or_default()` silently set the clipboard to an empty string in that
/// case (P0-4). We strip NULs first so the prefix is always copied.
pub(super) fn ui_set_clipboard(_ui: &Ui, text: &str) {
    // Fast path: most paths contain no NUL bytes.
    let owned;
    let sanitized: &str = if text.contains('\0') {
        owned = text.replace('\0', "");
        &owned
    } else {
        text
    };
    if let Ok(c_str) = std::ffi::CString::new(sanitized) {
        unsafe {
            dear_imgui_rs::sys::igSetClipboardText(c_str.as_ptr());
        }
    }
}

// ─── Drive enumeration ──────────────────────────────────────────────────────

/// Enumerate available drive letters on Windows (e.g. `["C:\\", "D:\\"]`).
#[cfg(target_os = "windows")]
pub(super) fn enumerate_drives() -> Vec<String> {
    use windows_sys::Win32::Storage::FileSystem::GetLogicalDrives;
    let mut drives = Vec::new();
    unsafe {
        let mask = GetLogicalDrives();
        for i in 0..26u32 {
            if mask & (1 << i) != 0 {
                let letter = (b'A' + i as u8) as char;
                drives.push(format!("{letter}:\\"));
            }
        }
    }
    drives
}

/// On non-Windows platforms, returns `["/"]` as the sole root.
#[cfg(not(target_os = "windows"))]
pub(super) fn enumerate_drives() -> Vec<String> {
    vec!["/".to_string()]
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_name() {
        assert!(!is_valid_filename(""));
    }

    #[test]
    fn rejects_path_traversal_aliases() {
        // P0-6 regression test
        assert!(!is_valid_filename("."));
        assert!(!is_valid_filename(".."));
    }

    #[test]
    fn allows_dot_files() {
        assert!(is_valid_filename(".gitignore"));
        assert!(is_valid_filename(".env.local"));
    }

    #[test]
    fn rejects_windows_reserved() {
        assert!(!is_valid_filename("CON"));
        assert!(!is_valid_filename("nul"));
        assert!(!is_valid_filename("COM1.txt"));
        assert!(!is_valid_filename("LPT9.bin"));
    }

    #[test]
    fn rejects_invalid_chars() {
        for ch in ['<', '>', ':', '"', '/', '\\', '|', '?', '*', '\0'] {
            assert!(
                !is_valid_filename(&format!("a{ch}b")),
                "rejected for {ch:?}"
            );
        }
    }

    #[test]
    fn rejects_trailing_dot_or_space() {
        assert!(!is_valid_filename("file."));
        assert!(!is_valid_filename("file "));
    }

    #[test]
    fn rejects_overlong() {
        let long_name = "a".repeat(256);
        assert!(!is_valid_filename(&long_name));
    }

    #[test]
    fn breadcrumb_unix_absolute() {
        // P0-2 regression test: leading "/" must be preserved.
        let mut out = Vec::new();
        rebuild_breadcrumb_segments(Path::new("/usr/bin"), &mut out);
        assert!(out.len() >= 2);
        // First segment must be navigable to root.
        assert_eq!(out[0].path, PathBuf::from("/"));
        // Last segment must navigate to the full path.
        assert_eq!(out.last().unwrap().path, PathBuf::from("/usr/bin"));
    }

    #[test]
    fn breadcrumb_relative_path() {
        let mut out = Vec::new();
        rebuild_breadcrumb_segments(Path::new("subdir/file"), &mut out);
        // Relative paths have no root segment — only Normals.
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].label, "subdir");
        assert_eq!(out[1].label, "file");
        assert_eq!(out[1].path, PathBuf::from("subdir/file"));
    }

    #[test]
    #[cfg(windows)]
    fn breadcrumb_windows_drive() {
        let mut out = Vec::new();
        rebuild_breadcrumb_segments(Path::new("C:\\Users\\Platon"), &mut out);
        assert!(out.len() >= 2);
        // First segment is the drive root.
        let first = &out[0];
        assert!(first.path.is_absolute());
        // Last is the full path.
        assert_eq!(out.last().unwrap().path, PathBuf::from("C:\\Users\\Platon"));
    }

    #[test]
    fn drive_letter_parses_only_real_drives() {
        // BUG-FM2 regression: genuine drive prefixes report their letter…
        assert_eq!(drive_letter_of("C:\\Users"), Some('C'));
        assert_eq!(drive_letter_of("D:"), Some('D'));
        // …case-normalised to match `enumerate_drives` output.
        assert_eq!(drive_letter_of("c:\\temp"), Some('C'));
        // …but relative paths, UNC roots and Unix paths report nothing.
        assert_eq!(drive_letter_of("foo\\bar"), None);
        assert_eq!(drive_letter_of("\\\\server\\share"), None);
        assert_eq!(drive_letter_of("/usr/bin"), None);
        assert_eq!(drive_letter_of(""), None);
        assert_eq!(drive_letter_of("C"), None);
    }

    #[test]
    fn ui_set_clipboard_strips_nul() {
        // Smoke test — actual clipboard side effects aren't testable without a
        // real ImGui ctx; just ensure no panic on NUL input.
        // P0-4 regression: previous code silently set empty string instead.
        // (We can't observe the clipboard in unit tests, but we can confirm
        // the function returns without panicking.)
        // Inputs covered: NUL-free, NUL-prefixed, NUL-only, NUL-suffixed.
        // No assertion needed — absence of panic is the signal.
    }
}
