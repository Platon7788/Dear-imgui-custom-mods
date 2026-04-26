//! wgpu + Dear ImGui setup and per-frame rendering.

use std::sync::Arc;

use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use pollster::block_on;
use winit::window::Window;

use super::config::PowerMode;

pub(super) fn init_wgpu(
    window: &Arc<Window>,
    power: PowerMode,
) -> (
    wgpu::Device,
    wgpu::Queue,
    wgpu::Surface<'static>,
    wgpu::SurfaceConfiguration,
) {
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
        .expect("wgpu: create_surface failed");

    let (adapter, device, queue) = pick_and_open_adapter(&instance, &surface, backends, power)
        .expect("wgpu: no usable adapter found (tried DX12, Vulkan, GL)");

    #[cfg(debug_assertions)]
    {
        let info = adapter.get_info();
        if info.device_type == wgpu::DeviceType::Cpu {
            eprintln!(
                "wgpu: WARNING — software renderer \"{}\" ({:?})",
                info.name, info.backend,
            );
        }
    }

    let phys = window.inner_size();
    let surface_caps = surface.get_capabilities(&adapter);
    let surface_format = surface_caps
        .formats
        .iter()
        .find(|&&f| {
            f == wgpu::TextureFormat::Bgra8UnormSrgb || f == wgpu::TextureFormat::Rgba8UnormSrgb
        })
        .copied()
        .or_else(|| surface_caps.formats.first().copied())
        .expect("wgpu: adapter reports no supported surface formats");

    // Prefer Opaque so the wgpu pixels are composited without any
    // alpha-channel transparency. CompositeAlphaMode::Auto can select
    // PreMultiplied on some Win10 DX12 configurations, which — when the
    // window is in DWM glass mode — would make the entire window appear
    // transparent.  Opaque is always available on DX12/Vulkan.
    let alpha_mode = surface_caps
        .alpha_modes
        .iter()
        .find(|&&m| m == wgpu::CompositeAlphaMode::Opaque)
        .copied()
        .unwrap_or(wgpu::CompositeAlphaMode::Auto);

    let surface_cfg = wgpu::SurfaceConfiguration {
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        format: surface_format,
        width: phys.width.max(1),
        height: phys.height.max(1),
        present_mode: wgpu::PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        alpha_mode,
        view_formats: vec![],
    };
    surface.configure(&device, &surface_cfg);

    (device, queue, surface, surface_cfg)
}

fn adapter_score(info: &wgpu::AdapterInfo, power: PowerMode) -> i32 {
    let device = match (info.device_type, power) {
        (wgpu::DeviceType::IntegratedGpu, PowerMode::LowPower) => 40,
        (wgpu::DeviceType::DiscreteGpu, PowerMode::LowPower) => 30,
        (wgpu::DeviceType::DiscreteGpu, _) => 40,
        (wgpu::DeviceType::IntegratedGpu, _) => 30,
        (wgpu::DeviceType::Other, _) => 20,
        (wgpu::DeviceType::VirtualGpu, _) => 10,
        (wgpu::DeviceType::Cpu, _) => 0,
    };
    let backend = match info.backend {
        wgpu::Backend::Dx12 => 4,
        wgpu::Backend::Vulkan => 3,
        wgpu::Backend::Metal => 3,
        wgpu::Backend::Gl => 1,
        _ => 0,
    };
    device + backend
}

fn pick_and_open_adapter(
    instance: &wgpu::Instance,
    surface: &wgpu::Surface<'_>,
    backends: wgpu::Backends,
    power: PowerMode,
) -> Option<(wgpu::Adapter, wgpu::Device, wgpu::Queue)> {
    let mut candidates: Vec<wgpu::Adapter> = block_on(instance.enumerate_adapters(backends))
        .into_iter()
        .filter(|a| a.is_surface_supported(surface))
        .filter(|a| {
            power != PowerMode::HighPerformance
                || a.get_info().device_type != wgpu::DeviceType::Cpu
        })
        .collect();
    candidates.sort_by_key(|a| std::cmp::Reverse(adapter_score(&a.get_info(), power)));
    for adapter in candidates {
        if let Ok((device, queue)) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
        {
            return Some((adapter, device, queue));
        }
    }
    None
}

pub(super) fn init_imgui(
    window: &Arc<Window>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_format: wgpu::TextureFormat,
    font_size: f32,
    titlebar_theme: crate::theme::Theme,
    merge_mdi_icons: bool,
) -> (dear_imgui_rs::Context, WinitPlatform, WgpuRenderer) {
    let mut context = dear_imgui_rs::Context::create();
    let _ = context.set_ini_filename(None::<std::path::PathBuf>);

    let mut platform = WinitPlatform::new(&mut context);
    platform.attach_window(window, HiDpiMode::Default, &mut context);

    let hidpi = (window.scale_factor() as f32).clamp(1.0, 3.0);
    let scaled_font = (font_size * hidpi).round();
    context.io_mut().set_font_global_scale(1.0 / hidpi);

    use crate::code_editor::BuiltinFont;
    context.fonts().add_font_from_memory_ttf(
        BuiltinFont::Hack.data(),
        scaled_font,
        Some(
            &dear_imgui_rs::FontConfig::new()
                .size_pixels(scaled_font)
                .oversample_h(2)
                .name("Hack"),
        ),
        None,
    );

    if merge_mdi_icons {
        crate::fonts::merge_mdi_icons(&mut context, scaled_font);
    }

    titlebar_theme.apply_imgui_style(context.style_mut());

    let renderer = WgpuRenderer::new(
        WgpuInitInfo::new(device, queue, surface_format),
        &mut context,
    )
    .expect("imgui-wgpu: renderer init failed");

    (context, platform, renderer)
}
