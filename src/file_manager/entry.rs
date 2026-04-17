//! Filesystem entry representation with pre-computed display strings.
//!
//! All display formatting happens once in [`FsEntry::from_dir_entry()`] during
//! directory refresh — not per-frame. This eliminates per-frame allocations
//! in the render loop.
//!
//! Also contains sorting logic ([`sort_entries()`]) and formatting helpers
//! ([`format_size()`]) used by the file manager.

use std::cmp::Ordering;
use std::path::PathBuf;
use std::time::SystemTime;

// ─── Sort ───────────────────────────────────────────────────────────────────

/// Which column the file list is sorted by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum SortColumn {
    /// Sort by file/folder name (case-insensitive).
    #[default]
    Name,
    /// Sort by file size in bytes.
    Size,
    /// Sort by last modification timestamp.
    DateModified,
    /// Sort by file extension, with secondary sort by name.
    Type,
}

/// Sort direction.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum SortOrder {
    /// A → Z, smallest → largest, oldest → newest.
    #[default]
    Ascending,
    /// Z → A, largest → smallest, newest → oldest.
    Descending,
}

// ─── FsEntry ────────────────────────────────────────────────────────────────

/// A single file or directory entry with pre-computed display strings.
#[derive(Clone)]
pub(super) struct FsEntry {
    /// Original filename as returned by the OS.
    pub name: String,
    /// Pre-computed lowercase name for sorting and search (zero per-call alloc).
    pub name_lower: String,
    /// Full absolute path to this entry.
    pub path: PathBuf,
    /// `true` for directories, `false` for files.
    pub is_dir: bool,
    /// File size in bytes (0 for directories).
    pub size: u64,
    /// Pre-formatted size string, e.g. "1.2 MB". Empty for directories.
    pub size_display: String,
    /// Last modification time (if available).
    pub date_modified: Option<SystemTime>,
    /// Pre-formatted date string, e.g. "2026-03-07 14:30".
    pub date_display: String,
    /// Lowercase file extension without dot, e.g. "rs". Empty for directories.
    pub extension: String,
    /// Display string for the Type column: extension or "Folder".
    pub type_display: String,
    /// Whether this entry is hidden (dotfile on Unix, hidden attribute on Windows).
    pub is_hidden: bool,
}

impl FsEntry {
    /// Build an `FsEntry` from a `std::fs::DirEntry`, pre-computing all display strings.
    pub(super) fn from_dir_entry(entry: &std::fs::DirEntry) -> Option<Self> {
        let meta = entry.metadata().ok()?;
        let name = entry.file_name().to_str()?.to_string();
        let is_dir = meta.is_dir();
        let size = if is_dir { 0 } else { meta.len() };

        let size_display = if is_dir {
            String::new()
        } else {
            format_size(size)
        };

        let date_modified = meta.modified().ok();
        let date_display = date_modified
            .map(format_system_time)
            .unwrap_or_default();

        let extension = if is_dir {
            String::new()
        } else {
            entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase()
        };

        let type_display = if is_dir {
            "Folder".to_string()
        } else if extension.is_empty() {
            "File".to_string()
        } else {
            extension.clone()
        };

        let is_hidden = is_hidden_entry(&name, &meta);

        let name_lower = name.to_lowercase();
        Some(Self {
            name,
            name_lower,
            path: entry.path(),
            is_dir,
            size,
            size_display,
            date_modified,
            date_display,
            extension,
            type_display,
            is_hidden,
        })
    }
}

// ─── Sorting ────────────────────────────────────────────────────────────────

/// Sort entries in-place by the given column and order.
///
/// When `dirs_first` is `true`, directories are grouped before files.
/// The Type column sorts by extension, with a secondary sort by name for entries
/// sharing the same extension.
pub(super) fn sort_entries(entries: &mut [FsEntry], column: SortColumn, order: SortOrder, dirs_first: bool) {
    entries.sort_by(|a, b| {
        // Optionally group directories before files
        if dirs_first {
            match b.is_dir.cmp(&a.is_dir) {
                Ordering::Equal => {}
                other => return other,
            }
        }

        let cmp = match column {
            SortColumn::Name => a.name_lower.cmp(&b.name_lower),
            SortColumn::Size => a.size.cmp(&b.size),
            SortColumn::DateModified => a.date_modified.cmp(&b.date_modified),
            SortColumn::Type => a
                .extension
                .cmp(&b.extension)
                .then_with(|| a.name_lower.cmp(&b.name_lower)),
        };

        match order {
            SortOrder::Ascending => cmp,
            SortOrder::Descending => cmp.reverse(),
        }
    });
}

// ─── Hidden file detection ───────────────────────────────────────────────

/// Check if a file is hidden (dotfile or Windows hidden attribute).
fn is_hidden_entry(name: &str, meta: &std::fs::Metadata) -> bool {
    // Dotfiles on all platforms
    if name.starts_with('.') {
        return true;
    }

    // Windows hidden attribute
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
        if meta.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0 {
            return true;
        }
    }

    #[cfg(not(target_os = "windows"))]
    let _ = meta;

    false
}

// ─── Formatting helpers ─────────────────────────────────────────────────────

/// Format a byte count into a human-readable string.
pub(super) fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Format a `SystemTime` as "YYYY-MM-DD HH:MM" (local time approximation).
fn format_system_time(time: SystemTime) -> String {
    // Use duration since UNIX_EPOCH and manual UTC calculation.
    // For a file dialog display, UTC is acceptable (no chrono dependency).
    let dur = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = dur.as_secs();

    // Days and time-of-day
    let days = secs / 86400;
    let day_secs = secs % 86400;
    let hour = day_secs / 3600;
    let minute = (day_secs % 3600) / 60;

    // Convert days since epoch to Y-M-D (civil calendar)
    let (year, month, day) = days_to_ymd(days);

    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

/// Convert days since 1970-01-01 to (year, month, day).
fn days_to_ymd(days: u64) -> (u64, u64, u64) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days + 719468;
    let era = z / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
