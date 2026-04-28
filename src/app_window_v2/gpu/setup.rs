//! wgpu instance / adapter / device / surface initialisation.
//!
//! Adapter selection mirrors `IMGUI_NXT`'s production-tested path:
//! `request_adapter(HighPerformance, &surface)` first, then a
//! `force_fallback_adapter: true` retry. Going through the OS GPU
//! manager (rather than enumerating + scoring ourselves) lets the
//! driver choose the display-routed adapter on hybrid laptops
//! (NVIDIA Optimus / AMD switchable graphics) — the previous
//! `enumerate_adapters` + manual score path could pick the discrete
//! dGPU even when the display is wired through the iGPU, leading to
//! failed `request_device` / black windows on integrated-only
//! machines.

use std::sync::Arc;

use pollster::block_on;
use winit::window::Window;

use super::super::config::{AppConfigV2, FpsModeV2, PowerModeV2};

pub(crate) fn init_wgpu(
    window: &Arc<Window>,
    cfg: &AppConfigV2,
) -> (
    wgpu::Instance,
    wgpu::Adapter,
    wgpu::Device,
    wgpu::Queue,
    wgpu::Surface<'static>,
    wgpu::SurfaceConfiguration,
) {
    // Backend preference: DX12 (Windows native) → Vulkan (newer GPUs) →
    // GL (last-resort compat path for old Intel iGPUs without DX12).
    // `WGPU_BACKEND` env var can force a specific backend at runtime.
    #[cfg(windows)]
    let backends = wgpu::Backends::DX12 | wgpu::Backends::VULKAN | wgpu::Backends::GL;
    #[cfg(not(windows))]
    let backends = wgpu::Backends::PRIMARY;

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let surface = instance
        .create_surface(window.clone())
        .expect("wgpu: create_surface");

    let adapter = pick_adapter(&instance, &surface, cfg.power_mode);
    log_adapter(&adapter.get_info());

    let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        .expect("wgpu: request_device");

    let phys = window.inner_size();
    let caps = surface.get_capabilities(&adapter);

    let format = caps
        .formats
        .iter()
        .find(|&&f| {
            matches!(
                f,
                wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Rgba8UnormSrgb
            )
        })
        .copied()
        .or_else(|| caps.formats.first().copied())
        .expect("wgpu: no supported surface format");

    // Pick a present_mode that is **actually advertised** by the adapter.
    // Some integrated GPUs do not support Mailbox / Immediate, so the
    // user's `Unlimited` preference must fall back to Fifo. Auto* modes
    // would do this internally on most drivers, but explicit validation
    // avoids panics on the rare paths where the resolver short-circuits.
    let present_mode = pick_present_mode(&caps.present_modes, &cfg.render_mode.fps_mode());

    // alpha_mode — `caps.alpha_modes` is guaranteed non-empty by spec.
    // Prefer `Opaque` (most efficient compositor path on Windows DWM);
    // fall back to whatever the adapter chose first (typically `Auto`).
    let alpha_mode = caps
        .alpha_modes
        .iter()
        .find(|&&m| m == wgpu::CompositeAlphaMode::Opaque)
        .copied()
        .or_else(|| caps.alpha_modes.first().copied())
        .unwrap_or(wgpu::CompositeAlphaMode::Auto);

    let surface_cfg = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format,
        width: phys.width.max(1),
        height: phys.height.max(1),
        present_mode,
        desired_maximum_frame_latency: frame_latency(cfg.power_mode),
        alpha_mode,
        view_formats: vec![],
    };
    surface.configure(&device, &surface_cfg);
    (instance, adapter, device, queue, surface, surface_cfg)
}

// ── Adapter selection ───────────────────────────────────────────────────────

/// Pick a usable adapter through the OS GPU manager.
///
/// First tries the system-preferred high-performance adapter
/// (discrete dGPU on hybrid laptops, dedicated card on desktops).
/// If that fails — common on integrated-only machines or when the
/// requested backend is unavailable — falls back to a software /
/// fallback adapter so the app still launches.
///
/// **Why not `enumerate_adapters` + manual score?** Enumerating
/// returns every visible adapter; on NVIDIA Optimus / AMD switchable
/// laptops the display is often routed through the iGPU even though
/// a dGPU is present. Picking the dGPU ourselves and then failing
/// `request_device` (or producing a black window) is a real bug we
/// hit. The OS GPU manager knows the routing and picks correctly.
fn pick_adapter(
    instance: &wgpu::Instance,
    surface: &wgpu::Surface<'_>,
    power: PowerModeV2,
) -> wgpu::Adapter {
    // Both branches go through `request_adapter(compatible_surface)` —
    // the OS GPU manager handles the actual routing. On integrated-only
    // laptops `HighPerformance` returns the iGPU (only adapter). On
    // hybrid laptops the OS picks the display-attached GPU correctly.
    let preference = match power {
        PowerModeV2::LowPower => wgpu::PowerPreference::LowPower,
        PowerModeV2::HighPerformance => wgpu::PowerPreference::HighPerformance,
    };

    let primary = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: preference,
        compatible_surface: Some(surface),
        force_fallback_adapter: false,
    }));

    if let Ok(a) = primary {
        return a;
    }

    // Primary failed (no DX12-capable adapter, no compatible surface
    // provider, etc.) — fall through to the WARP / llvmpipe software
    // fallback. UX hit but app still launches.
    block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::None,
        compatible_surface: Some(surface),
        force_fallback_adapter: true,
    }))
    .expect("wgpu: no usable adapter (primary + fallback both failed)")
}

// ── Present-mode validation ─────────────────────────────────────────────────

/// Pick a [`wgpu::PresentMode`] that the adapter actually advertises.
///
/// NxT proved that hard-coded `Fifo` works on every Windows GPU we've
/// tested (Intel iGPU, NVIDIA discrete, AMD discrete, software fallback).
/// We respect the user's `FpsModeV2::Unlimited` preference by trying
/// `Mailbox` / `Immediate` first, then falling back through `FifoRelaxed`
/// to `Fifo`. `Fifo` is mandated by the wgpu spec to be supported on
/// every surface, so the final fallback is guaranteed.
fn pick_present_mode(available: &[wgpu::PresentMode], fps: &FpsModeV2) -> wgpu::PresentMode {
    let supports = |m: wgpu::PresentMode| available.contains(&m);

    match fps {
        FpsModeV2::Unlimited => {
            // Triple-buffer with no vsync where possible.
            if supports(wgpu::PresentMode::Mailbox) {
                wgpu::PresentMode::Mailbox
            } else if supports(wgpu::PresentMode::Immediate) {
                wgpu::PresentMode::Immediate
            } else {
                wgpu::PresentMode::Fifo
            }
        }
        _ => {
            // Adaptive vsync (smooth on slow frames) → strict vsync.
            if supports(wgpu::PresentMode::FifoRelaxed) {
                wgpu::PresentMode::FifoRelaxed
            } else {
                wgpu::PresentMode::Fifo
            }
        }
    }
}

// ── Diagnostics ─────────────────────────────────────────────────────────────

fn log_adapter(info: &wgpu::AdapterInfo) {
    let line = format!(
        "wgpu: adapter \"{}\" ({:?}, {:?}) | driver \"{}\" \"{}\"",
        info.name, info.device_type, info.backend, info.driver, info.driver_info,
    );
    eprintln!("{line}");
    crate::app_window_v2::win32_debug_log(&line);
    if info.device_type == wgpu::DeviceType::Cpu {
        let warn = "wgpu: WARNING: software renderer active — expect poor performance";
        eprintln!("{warn}");
        crate::app_window_v2::win32_debug_log(warn);
    }
}

fn frame_latency(_power: PowerModeV2) -> u32 {
    // 2 = standard double-buffer. Tuning per power mode risks CPU stalls on weak
    // hardware (latency=1) or visible lag (latency=3) — not worth it for a UI toolkit.
    2
}
