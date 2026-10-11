use super::*;
#[test]
fn legacy_config_without_background_or_extension_policy_loads() {
    let ron = include_str!("config.ron")
        .replace("    background_listing: false,", "")
        .replace("    save_extension_from_filter: false,", "");
    let config: FileManagerConfig = ron::from_str(&ron).unwrap();
    assert!(!config.background_listing);
    assert!(!config.save_extension_from_filter);
}
#[test]
fn extension_tracks_type_and_does_not_sanitize_invalid_names() {
    let mut fm = FileManager::new_with_config(FileManagerConfig {
        save_extension_from_filter: true,
        ..Default::default()
    });
    fm.save_file(
        None,
        "capture.json",
        vec![
            FileFilter::new("JSON", ["json"]),
            FileFilter::new("TOML", ["toml"]),
        ],
    );
    fm.set_active_filter(1);
    assert_eq!(fm.filename_buf, "capture.toml");
    fm.filename_buf = "../escape.json".into();
    fm.normalize_save_extension();
    assert_eq!(fm.filename_buf, "../escape.json");
}
#[test]
fn background_failure_preserves_directory_and_history() {
    let mut fm = FileManager::new_with_config(FileManagerConfig {
        background_listing: true,
        ..Default::default()
    });
    let previous = fm.current_path.clone();
    fm.open_folder(Some(previous.join("not-existing-nxt-directory")));
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while fm.listing_pending() && std::time::Instant::now() < deadline {
        fm.poll_listing();
        std::thread::yield_now();
    }
    assert!(!fm.listing_pending());
    assert_eq!(fm.current_path, previous);
    assert!(fm.error.is_some());
    assert!(fm.history.peek_back().is_none());
}

fn drain(fm: &mut FileManager) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while fm.listing_pending() && std::time::Instant::now() < deadline {
        fm.poll_listing();
        std::thread::yield_now();
    }
    assert!(!fm.listing_pending());
}
#[test]
fn changing_filter_during_initial_scan_preserves_requested_directory() {
    let dir = tempfile::tempdir().unwrap();
    let mut fm = FileManager::new_with_config(FileManagerConfig {
        background_listing: true,
        save_extension_from_filter: true,
        ..Default::default()
    });
    fm.save_file(
        Some(dir.path().to_path_buf()),
        "capture.json",
        vec![
            FileFilter::new("JSON", ["json"]),
            FileFilter::new("TOML", ["toml"]),
        ],
    );
    fm.set_active_filter(1);
    drain(&mut fm);
    assert_eq!(fm.current_path, dir.path());
    assert_eq!(fm.filename_buf, "capture.toml");
    assert!(fm.history.peek_back().is_none());
    assert!(fm.error.is_none());
}
#[test]
fn latest_navigation_wins_and_only_success_commits_history() {
    let mut fm = FileManager::new_with_config(FileManagerConfig {
        background_listing: true,
        ..Default::default()
    });
    let initial = fm.current_path.clone();
    for _ in 0..100 {
        fm.request_listing(initial.join("missing-nxt-directory"), Navigation::Push);
    }
    fm.request_listing(initial.clone(), Navigation::Refresh);
    drain(&mut fm);
    assert!(fm.error.is_none());
    assert_eq!(fm.current_path, initial);
    assert!(fm.history.peek_back().is_none());
    assert!(fm.background.as_ref().unwrap().queued.is_none());
}
#[test]
fn background_save_probe_distinguishes_file_directory_and_missing() {
    let mut fm = FileManager::new_with_config(FileManagerConfig {
        background_listing: true,
        ..Default::default()
    });
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("capture.toml");
    fm.request_save_check(file.clone());
    drain(&mut fm);
    assert!(fm.save_ready);
    assert_eq!(fm.selected_path.as_ref(), Some(&file));
    fm.save_ready = false;
    std::fs::write(&file, b"previous").unwrap();
    fm.request_save_check(file.clone());
    drain(&mut fm);
    assert!(!fm.save_ready);
    assert!(fm.show_overwrite_confirm);
    assert!(!fm.overwrite_confirmed);
    fm.filename_buf = "different.toml".into();
    fm.mode = DialogMode::SaveFile;
    fm.finalize_selection();
    assert!(fm.overwrite_confirmed);
    assert_eq!(fm.selected_path.as_ref(), Some(&file));
    fm.request_save_check(dir.path().to_path_buf());
    drain(&mut fm);
    assert!(fm.error.is_some());
}

#[test]
fn audit_mutation_is_not_lost_to_navigation_and_uses_frozen_directory() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    std::fs::create_dir(&first).unwrap();
    std::fs::create_dir(&second).unwrap();
    let mut fm = FileManager::new_with_config(FileManagerConfig {
        background_listing: true,
        ..Default::default()
    });
    fm.open_folder(Some(first.clone()));
    drain(&mut fm);
    fm.create_folder("created");
    assert!(fm.mutation_pending());
    for _ in 0..100 {
        fm.request_listing(second.clone(), Navigation::Push);
    }
    drain(&mut fm);
    assert!(first.join("created").is_dir());
    assert!(!second.join("created").exists());
    assert_eq!(fm.current_path, second);
    assert!(!fm.mutation_pending());
    assert_eq!(fm.history.peek_back(), Some(first.as_path()));
}

#[test]
fn audit_background_create_rename_delete_and_error_preserve_files() {
    let dir = tempfile::tempdir().unwrap();
    let mut fm = FileManager::new_with_config(FileManagerConfig {
        background_listing: true,
        ..Default::default()
    });
    fm.open_file(Some(dir.path().to_path_buf()), vec![]);
    drain(&mut fm);
    fm.create_file("first.txt");
    drain(&mut fm);
    std::fs::write(dir.path().join("first.txt"), "original").unwrap();
    fm.create_file("first.txt");
    drain(&mut fm);
    assert!(fm.error.is_some());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("first.txt")).unwrap(),
        "original"
    );
    let index = fm
        .entries
        .iter()
        .position(|e| e.name == "first.txt")
        .unwrap();
    fm.rename_entry(index, "renamed.txt");
    drain(&mut fm);
    assert!(!dir.path().join("first.txt").exists());
    let index = fm
        .entries
        .iter()
        .position(|e| e.name == "renamed.txt")
        .unwrap();
    fm.delete_entry(index);
    drain(&mut fm);
    assert!(!dir.path().join("renamed.txt").exists());
    assert!(fm.entries.is_empty());
}

#[test]
fn audit_failed_mutation_still_serves_new_navigation_and_surfaces_error() {
    let dir = tempfile::tempdir().unwrap();
    let other = dir.path().join("other");
    std::fs::create_dir(&other).unwrap();
    let original = dir.path().join("existing.txt");
    std::fs::write(&original, "original").unwrap();
    let mut fm = FileManager::new_with_config(FileManagerConfig {
        background_listing: true,
        ..Default::default()
    });
    fm.open_file(Some(dir.path().to_path_buf()), vec![]);
    drain(&mut fm);
    fm.create_file("existing.txt");
    fm.request_listing(other.clone(), Navigation::Push);
    drain(&mut fm);
    assert_eq!(fm.current_path, other);
    assert!(matches!(fm.error, Some(FmError::CreateFileFailed(_))));
    assert_eq!(std::fs::read_to_string(original).unwrap(), "original");
}
