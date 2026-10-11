use super::*;
fn finish_background(fm: &mut FileManager) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while fm.listing_pending() && std::time::Instant::now() < deadline {
        fm.poll_listing();
        std::thread::yield_now();
    }
    assert!(!fm.listing_pending());
}
#[test]
fn escape_wins_over_ready_save_but_uncancelled_save_confirms() {
    let _guard = crate::IMGUI_CTX_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut ctx = dear_imgui_rs::Context::create();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    ctx.io_mut().set_display_size([1280.0, 800.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("capture.json");
    let mut fm = FileManager::new_with_config(super::super::FileManagerConfig {
        background_listing: true,
        ..Default::default()
    });
    for cancel in [true, false] {
        fm.save_file(Some(dir.path().to_path_buf()), "capture.json", vec![]);
        finish_background(&mut fm);
        ctx.io_mut().add_key_event(Key::Escape, false);
        assert!(!fm.render(ctx.frame()));
        let _ = ctx.render_legacy();
        fm.request_save_check(target.clone());
        finish_background(&mut fm);
        assert!(fm.save_ready);
        ctx.io_mut().add_key_event(Key::Escape, cancel);
        assert_eq!(fm.render(ctx.frame()), !cancel);
        let _ = ctx.render_legacy();
        assert!(!fm.is_open);
        assert_eq!(fm.selected_path.as_ref(), Some(&target));
        assert!(!target.exists());
    }
}
#[test]
fn audit_locale_change_keeps_open_save_modal_and_confirmation() {
    let _guard = crate::IMGUI_CTX_TEST_LOCK
        .lock()
        .unwrap_or_else(|e| e.into_inner());
    let mut ctx = dear_imgui_rs::Context::create();
    let _ = ctx.set_ini_filename::<std::path::PathBuf>(None);
    ctx.io_mut().set_display_size([1280.0, 800.0]);
    ctx.io_mut().set_delta_time(1.0 / 60.0);
    ctx.font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    let dir = tempfile::tempdir().unwrap();
    let mut fm = FileManager::new_with_config(super::super::FileManagerConfig {
        background_listing: true,
        ..Default::default()
    });
    fm.save_file(Some(dir.path().to_path_buf()), "capture.json", vec![]);
    finish_background(&mut fm);
    assert!(!fm.render(ctx.frame()));
    let _ = ctx.render_legacy();
    fm.set_locale(crate::i18n::Locale::Ru);
    fm.request_save_check(dir.path().join("capture.json"));
    finish_background(&mut fm);
    assert!(
        fm.render(ctx.frame()),
        "changing the label must not change popup identity"
    );
    let _ = ctx.render_legacy();
    assert!(!fm.is_open);
}
