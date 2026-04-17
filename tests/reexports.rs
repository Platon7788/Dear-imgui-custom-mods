//! Smoke tests for the foundational crate re-exports.
//!
//! `dear-imgui-custom-mod` re-exports `dear-imgui-rs`, `dear-imgui-wgpu`,
//! `dear-imgui-winit`, `wgpu`, `winit` so downstream users don't need to
//! duplicate-pin them. If one of the re-exports breaks, downstream builds
//! start failing in confusing ways — these tests catch that early.

#[test]
fn reexports_are_reachable() {
    // Just prove the paths resolve and the underlying types haven't been
    // moved or renamed under us. No runtime behavior required.
    use dear_imgui_custom_mod::{dear_imgui_rs, dear_imgui_wgpu, dear_imgui_winit, wgpu, winit};

    // dear_imgui_rs::Ui is the central type — users always need it.
    let _: Option<&dear_imgui_rs::Ui> = None;

    // The three platform-facing types one-liner-style — if any of these
    // stops being re-exported from the upstream crate, this test breaks
    // at compile time rather than at the consumer's link step.
    let _: Option<wgpu::Backends> = None;
    let _: Option<winit::event::WindowEvent> = None;
    let _: Option<&dear_imgui_wgpu::WgpuRenderer> = None;
    let _: Option<&dear_imgui_winit::WinitPlatform> = None;
}

#[test]
fn theme_is_publicly_reachable() {
    // The most-renamed type during the 0.7→0.8 migration. Make sure the
    // canonical path still resolves.
    let _: dear_imgui_custom_mod::theme::Theme = dear_imgui_custom_mod::theme::Theme::Dark;
}
