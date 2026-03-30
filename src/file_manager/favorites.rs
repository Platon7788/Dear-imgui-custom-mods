//! Favorites sidebar — quick access to well-known and bookmarked folders.
//!
//! The [`FavoritesPanel`] is created with OS-specific defaults (Desktop, Documents,
//! Downloads) and can be extended with custom bookmarks via [`add_bookmark()`](FavoritesPanel::add_bookmark).
//!
//! Access the panel through [`FileManager::favorites_mut()`](super::FileManager::favorites_mut).

use std::path::PathBuf;

use crate::icons;

/// A single favorite/bookmark entry.
#[derive(Clone, Debug)]
pub struct FavoriteEntry {
    /// Display name shown in the sidebar, e.g. "Desktop".
    pub label: String,
    /// Absolute path to the folder.
    pub path: PathBuf,
    /// MDI icon codepoint string, e.g. `icons::MONITOR`.
    pub icon: &'static str,
}

/// Favorites panel state — a list of quick-access folder entries.
///
/// Rendered as a sidebar in the file manager dialog. Each entry is a clickable
/// row with an icon and label that navigates to the corresponding directory.
pub struct FavoritesPanel {
    /// Ordered list of favorite entries. Rendered top-to-bottom.
    pub entries: Vec<FavoriteEntry>,
}

impl FavoritesPanel {
    /// Create a panel with OS-specific well-known folders.
    pub fn with_defaults() -> Self {
        let mut entries = Vec::new();

        #[cfg(target_os = "windows")]
        {
            if let Ok(profile) = std::env::var("USERPROFILE") {
                let profile = PathBuf::from(profile);
                let candidates = [
                    ("Desktop", icons::MONITOR, "Desktop"),
                    ("Documents", icons::FILE_DOCUMENT, "Documents"),
                    ("Downloads", icons::TRAY_ARROW_DOWN, "Downloads"),
                ];
                for (label, icon, subdir) in candidates {
                    let path = profile.join(subdir);
                    if path.is_dir() {
                        entries.push(FavoriteEntry {
                            label: label.to_string(),
                            path,
                            icon,
                        });
                    }
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(home) = std::env::var("HOME") {
                let home = PathBuf::from(home);
                entries.push(FavoriteEntry {
                    label: "Home".to_string(),
                    path: home.clone(),
                    icon: icons::HOME,
                });
                for (label, icon, subdir) in [
                    ("Desktop", icons::MONITOR, "Desktop"),
                    ("Documents", icons::FILE_DOCUMENT, "Documents"),
                    ("Downloads", icons::TRAY_ARROW_DOWN, "Downloads"),
                ] {
                    let path = home.join(subdir);
                    if path.is_dir() {
                        entries.push(FavoriteEntry {
                            label: label.to_string(),
                            path,
                            icon,
                        });
                    }
                }
            }
        }

        Self { entries }
    }

    /// Add a custom bookmark.
    pub fn add_bookmark(&mut self, label: String, path: PathBuf) {
        self.entries.push(FavoriteEntry {
            label,
            path,
            icon: icons::STAR,
        });
    }

    /// Remove a bookmark by index.
    pub fn remove_bookmark(&mut self, index: usize) {
        if index < self.entries.len() {
            self.entries.remove(index);
        }
    }
}

impl Default for FavoritesPanel {
    fn default() -> Self {
        Self::with_defaults()
    }
}
