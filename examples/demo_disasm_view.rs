//! Demo: DisasmView — disassembly viewer showcase.
//!
//! Demonstrates branch arrows, breakpoint markers, block tinting,
//! syntax coloring, keyboard navigation, context menu, and all config options.
//!
//! Run: cargo run --example demo_disasm_view

use dear_imgui_custom_mod::disasm_view::{
    DisasmDataProvider, DisasmView, FlowKind, InstructionEntry, VecDisasmProvider,
};
use dear_imgui_rs::{Condition, StyleColor, Ui};
use dear_imgui_wgpu::{WgpuInitInfo, WgpuRenderer};
use dear_imgui_winit::{HiDpiMode, WinitPlatform};
use pollster::block_on;
use std::sync::Arc;
use winit::{
    application::ApplicationHandler,
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::Window,
};

// ─── Sample instructions ─────────────────────────────────────────────────────

/// Generate a realistic x86-64 function disassembly for demo purposes.
fn sample_instructions() -> Vec<InstructionEntry> {
    let mut instrs = Vec::new();
    let mut addr: u64 = 0x0040_1000;
    let mut block = 0usize;

    // ── Function prologue (block 0) ─────────────────────────
    instrs.push(InstructionEntry::new(addr, vec![0x55], "push", "rbp")
        .with_flow(FlowKind::Stack).with_block(block));
    addr += 1;
    instrs.push(InstructionEntry::new(addr, vec![0x48, 0x89, 0xE5], "mov", "rbp, rsp")
        .with_block(block));
    addr += 3;
    instrs.push(InstructionEntry::new(addr, vec![0x48, 0x83, 0xEC, 0x30], "sub", "rsp, 0x30")
        .with_flow(FlowKind::Stack).with_block(block));
    addr += 4;
    instrs.push(InstructionEntry::new(addr, vec![0x48, 0x89, 0x7D, 0xD8], "mov", "qword ptr [rbp-0x28], rdi")
        .with_block(block).with_comment("save arg0"));
    addr += 4;
    instrs.push(InstructionEntry::new(addr, vec![0x89, 0x75, 0xD4], "mov", "dword ptr [rbp-0x2C], esi")
        .with_block(block).with_comment("save arg1"));
    addr += 3;

    // ── Null check (block 0) ────────────────────────────────
    instrs.push(InstructionEntry::new(addr, vec![0x48, 0x83, 0x7D, 0xD8, 0x00], "cmp", "qword ptr [rbp-0x28], 0")
        .with_block(block));
    addr += 5;
    let je_target = addr + 0x2A; // jump to error block
    instrs.push(InstructionEntry::new(addr, vec![0x0F, 0x84, 0x26, 0x00, 0x00, 0x00], "je", format!("0x{:X}", je_target))
        .with_flow(FlowKind::Jump).with_target(je_target).with_block(block)
        .with_comment("jump if null"));
    addr += 6;

    // ── Main logic (block 1) ────────────────────────────────
    block = 1;
    instrs.push(InstructionEntry::new(addr, vec![0x48, 0x8B, 0x45, 0xD8], "mov", "rax, qword ptr [rbp-0x28]")
        .with_block(block));
    addr += 4;
    instrs.push(InstructionEntry::new(addr, vec![0x8B, 0x00], "mov", "eax, dword ptr [rax]")
        .with_block(block).with_comment("dereference ptr"));
    addr += 2;
    instrs.push(InstructionEntry::new(addr, vec![0x03, 0x45, 0xD4], "add", "eax, dword ptr [rbp-0x2C]")
        .with_block(block));
    addr += 3;
    instrs.push(InstructionEntry::new(addr, vec![0x89, 0x45, 0xFC], "mov", "dword ptr [rbp-0x4], eax")
        .with_block(block).with_comment("result"));
    addr += 3;

    // ── Range check (block 1) ───────────────────────────────
    instrs.push(InstructionEntry::new(addr, vec![0x83, 0x7D, 0xFC, 0x64], "cmp", "dword ptr [rbp-0x4], 0x64")
        .with_block(block));
    addr += 4;
    let jle_target = addr + 0x12; // skip clamp
    instrs.push(InstructionEntry::new(addr, vec![0x7E, 0x10], "jle", format!("0x{:X}", jle_target))
        .with_flow(FlowKind::Jump).with_target(jle_target).with_block(block)
        .with_comment("skip if <= 100"));
    addr += 2;

    // ── Clamp block (block 2) ───────────────────────────────
    block = 2;
    instrs.push(InstructionEntry::new(addr, vec![0xC7, 0x45, 0xFC, 0x64, 0x00, 0x00, 0x00], "mov", "dword ptr [rbp-0x4], 0x64")
        .with_block(block).with_comment("clamp to 100"));
    addr += 7;
    let call_target = 0x0040_1200;
    instrs.push(InstructionEntry::new(addr, vec![0x48, 0x8D, 0x3D, 0x50, 0x01, 0x00, 0x00], "lea", "rdi, [rip+0x150]")
        .with_block(block).with_comment("\"clamped!\""));
    addr += 7;
    instrs.push(InstructionEntry::new(addr, vec![0xE8, 0x00, 0x02, 0x00, 0x00], "call", format!("0x{:X}", call_target))
        .with_flow(FlowKind::Call).with_target(call_target).with_block(block)
        .with_comment("log_warning"));
    addr += 5;

    // ── After clamp / skip target (block 3) ─────────────────
    block = 3;
    // jle_target lands here
    instrs.push(InstructionEntry::new(addr, vec![0x8B, 0x45, 0xFC], "mov", "eax, dword ptr [rbp-0x4]")
        .with_block(block).with_comment("load result"));
    addr += 3;

    // ── Return path (block 3) ───────────────────────────────
    instrs.push(InstructionEntry::new(addr, vec![0x48, 0x83, 0xC4, 0x30], "add", "rsp, 0x30")
        .with_flow(FlowKind::Stack).with_block(block));
    addr += 4;
    instrs.push(InstructionEntry::new(addr, vec![0x5D], "pop", "rbp")
        .with_flow(FlowKind::Stack).with_block(block));
    addr += 1;
    instrs.push(InstructionEntry::new(addr, vec![0xC3], "ret", "")
        .with_flow(FlowKind::Return).with_block(block));
    addr += 1;

    // ── Padding ─────────────────────────────────────────────
    for _ in 0..3 {
        instrs.push(InstructionEntry::new(addr, vec![0xCC], "int3", "")
            .with_flow(FlowKind::Nop).with_block(block));
        addr += 1;
    }

    // ── Error handler (block 4) — je_target ─────────────────
    block = 4;
    instrs.push(InstructionEntry::new(addr, vec![0x48, 0x8D, 0x3D, 0x80, 0x01, 0x00, 0x00], "lea", "rdi, [rip+0x180]")
        .with_block(block).with_comment("\"null pointer!\""));
    addr += 7;
    instrs.push(InstructionEntry::new(addr, vec![0xE8, 0x20, 0x02, 0x00, 0x00], "call", format!("0x{:X}", 0x0040_1300))
        .with_flow(FlowKind::Call).with_target(0x0040_1300).with_block(block)
        .with_comment("log_error"));
    addr += 5;
    instrs.push(InstructionEntry::new(addr, vec![0xB8, 0xFF, 0xFF, 0xFF, 0xFF], "mov", "eax, 0xFFFFFFFF")
        .with_block(block).with_comment("return -1"));
    addr += 5;
    instrs.push(InstructionEntry::new(addr, vec![0xEB, 0xD0], "jmp", format!("0x{:X}", addr - 0x30))
        .with_flow(FlowKind::Jump).with_target(addr - 0x30).with_block(block)
        .with_comment("goto epilogue"));
    addr += 2;

    // ── Second function — simple leaf (block 5) ─────────────
    block = 5;
    instrs.push(InstructionEntry::new(addr, vec![0x90], "nop", "")
        .with_flow(FlowKind::Nop).with_block(block));
    addr += 1;

    // syscall example
    instrs.push(InstructionEntry::new(addr, vec![0x48, 0xC7, 0xC0, 0x3C, 0x00, 0x00, 0x00], "mov", "rax, 0x3C")
        .with_block(block).with_comment("SYS_exit"));
    addr += 7;
    instrs.push(InstructionEntry::new(addr, vec![0x48, 0x31, 0xFF], "xor", "rdi, rdi")
        .with_block(block).with_comment("exit code 0"));
    addr += 3;
    instrs.push(InstructionEntry::new(addr, vec![0x0F, 0x05], "syscall", "")
        .with_flow(FlowKind::System).with_block(block));
    let _ = addr + 2; // end of instructions

    // Set numbered breakpoints for demo.
    instrs[6].breakpoint = true;   // je (null check)
    instrs[6].bp_number = 1;
    instrs[16].breakpoint = true;  // call log_warning
    instrs[16].bp_number = 2;
    instrs[24].breakpoint = true;  // call log_error
    instrs[24].bp_number = 3;

    // Mark one instruction as current execution point.
    instrs[8].current = true;

    instrs
}

// ─── Demo state ──────────────────────────────────────────────────────────────

struct DemoState {
    view: DisasmView,
    provider: VecDisasmProvider,
    show_config: bool,
}

impl DemoState {
    fn new() -> Self {
        let instructions = sample_instructions();
        let provider = VecDisasmProvider::from_vec(instructions);
        let mut view = DisasmView::new("##disasm_demo");
        view.select(0);

        Self {
            view,
            provider,
            show_config: true,
        }
    }

    fn render(&mut self, ui: &Ui) {
        ui.window("DisasmView Demo")
            .size([1200.0, 750.0], Condition::FirstUseEver)
            .build(|| {
                self.render_toolbar(ui);
                ui.separator();

                let avail = ui.content_region_avail();
                let config_w = if self.show_config { 220.0 } else { 0.0 };
                let viewer_w = avail[0] - config_w - if self.show_config { 8.0 } else { 0.0 };

                ui.child_window("##dv_col")
                    .size([viewer_w, avail[1]])
                    .build(ui, || {
                        self.view.render(ui, &mut self.provider);
                    });

                if self.show_config {
                    ui.same_line();
                    ui.child_window("##cfg_col")
                        .size([config_w, avail[1]])
                        .build(ui, || {
                            self.render_config(ui);
                        });
                }
            });
    }

    fn render_toolbar(&mut self, ui: &Ui) {
        let sel_idx = self.view.selected_index();
        let count = self.provider.instruction_count();

        if let Some(idx) = sel_idx {
            if let Some(instr) = self.provider.instruction(idx) {
                ui.text(format!(
                    "Addr: 0x{:X}  |  {} {}  |  {:?}  |  Instr {}/{}",
                    instr.address(), instr.mnemonic(), instr.operands(),
                    instr.flow_kind(), idx + 1, count,
                ));
            }
        } else {
            ui.text(format!("No selection  |  {} instructions", count));
        }

        ui.same_line_with_pos(avail_right(ui, 80.0));
        ui.checkbox("Config", &mut self.show_config);

        // Quick-nav buttons.
        if ui.button("Top") {
            self.view.select(0);
        }
        ui.same_line();
        if ui.button("Bottom") {
            if count > 0 { self.view.select(count - 1); }
        }
        ui.same_line();
        if ui.button("Current (IP)") {
            for i in 0..count {
                if let Some(instr) = self.provider.instruction(i) {
                    if instr.is_current() {
                        self.view.select(i);
                        break;
                    }
                }
            }
        }
        ui.same_line();
        if ui.button("Breakpoint") {
            for i in 0..count {
                if let Some(instr) = self.provider.instruction(i) {
                    if instr.has_breakpoint() {
                        self.view.select(i);
                        break;
                    }
                }
            }
        }
    }

    fn render_config(&mut self, ui: &Ui) {
        ui.text("Configuration");
        ui.separator();

        ui.text("Display");
        ui.checkbox("Show Bytes", &mut self.view.config.show_bytes);
        ui.checkbox("Show Comments", &mut self.view.config.show_comments);
        ui.checkbox("Show Arrows", &mut self.view.config.show_arrows);
        ui.checkbox("Show Breakpoints", &mut self.view.config.show_breakpoints);
        ui.checkbox("Show Block Tints", &mut self.view.config.show_block_tints);
        ui.checkbox("Show Header", &mut self.view.config.show_header);
        ui.checkbox("Uppercase", &mut self.view.config.uppercase);
        ui.checkbox("64-bit Addresses", &mut self.view.config.address_width_64);

        ui.spacing();
        ui.separator();
        ui.text("Behavior");
        ui.checkbox("Editable", &mut self.view.config.editable);
        ui.checkbox("Follow Execution", &mut self.view.config.follow_execution);

        ui.spacing();
        ui.separator();
        ui.text("Color Legend");
        let c = &self.view.config.colors;
        ui.text_colored(c.mnemonic_normal, "Normal (mov, add)");
        ui.text_colored(c.mnemonic_jump, "Jump (je, jmp)");
        ui.text_colored(c.mnemonic_call, "Call");
        ui.text_colored(c.mnemonic_return, "Return (ret)");
        ui.text_colored(c.mnemonic_nop, "Nop / int3");
        ui.text_colored(c.mnemonic_stack, "Stack (push, pop)");
        ui.text_colored(c.mnemonic_system, "System (syscall)");
        ui.text_colored(c.mnemonic_invalid, "Invalid");

        ui.spacing();
        ui.text("Operand Colors");
        ui.text_colored(c.operand_register, "Register (rax)");
        ui.text_colored(c.operand_number, "Number (0x30)");
        ui.text_colored(c.operand_memory, "Memory ([rbp])");
        ui.text_colored(c.operand_string, "String");

        ui.spacing();
        ui.separator();
        ui.text_disabled("Up/Down: Navigate");
        ui.text_disabled("Enter: Follow branch");
        ui.text_disabled("G: Goto address");
        ui.text_disabled("F9: Toggle breakpoint");
        ui.text_disabled("Ctrl+C: Copy");
        ui.text_disabled("Alt+</>: Nav history");
        ui.text_disabled("Right-click: Context menu");
    }
}

fn avail_right(ui: &Ui, w: f32) -> f32 {
    ui.cursor_pos()[0] + ui.content_region_avail()[0] - w
}

// ─── wgpu + winit + imgui boilerplate ────────────────────────────────────────

struct GpuState {
    device: wgpu::Device,
    queue: wgpu::Queue,
    window: Arc<Window>,
    surface_cfg: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    context: dear_imgui_rs::Context,
    platform: WinitPlatform,
    renderer: WgpuRenderer,
    demo: DemoState,
}

struct App {
    gpu: Option<GpuState>,
}

impl App {
    fn new() -> Self { Self { gpu: None } }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() { return; }

        let window = Arc::new(
            event_loop
                .create_window(
                    Window::default_attributes()
                        .with_inner_size(LogicalSize::new(1200.0, 750.0))
                        .with_title("DisasmView Demo"),
                )
                .expect("window"),
        );

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance.create_surface(window.clone()).expect("surface");
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("adapter");
        let (device, queue) =
            block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("device");

        let phys = window.inner_size();
        let surface_cfg = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: phys.width.max(1),
            height: phys.height.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![wgpu::TextureFormat::Bgra8Unorm],
        };
        surface.configure(&device, &surface_cfg);

        let mut context = dear_imgui_rs::Context::create();
        let _ = context.set_ini_filename(None::<std::path::PathBuf>);

        let mut platform = WinitPlatform::new(&mut context);
        platform.attach_window(&window, HiDpiMode::Default, &mut context);

        let hidpi = window.scale_factor() as f32;
        let font_size = 15.0 * hidpi;
        context.io_mut().set_font_global_scale(1.0 / hidpi);

        use dear_imgui_custom_mod::code_editor::BuiltinFont;
        let cfg = dear_imgui_rs::FontConfig::new()
            .size_pixels(font_size)
            .oversample_h(2)
            .name("Hack");
        context.fonts().add_font_from_memory_ttf(
            BuiltinFont::Hack.data(),
            font_size,
            Some(&cfg),
            None,
        );

        apply_dark_theme(context.style_mut());

        let renderer = WgpuRenderer::new(
            WgpuInitInfo::new(device.clone(), queue.clone(), surface_cfg.format),
            &mut context,
        )
        .expect("renderer");

        self.gpu = Some(GpuState {
            device,
            queue,
            window,
            surface_cfg,
            surface,
            context,
            platform,
            renderer,
            demo: DemoState::new(),
        });
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(gpu) = self.gpu.as_mut() else { return };

        gpu.platform.handle_event::<()>(
            &mut gpu.context,
            &gpu.window,
            &Event::WindowEvent { window_id, event: event.clone() },
        );

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                gpu.surface_cfg.width = new_size.width.max(1);
                gpu.surface_cfg.height = new_size.height.max(1);
                gpu.surface.configure(&gpu.device, &gpu.surface_cfg);
                gpu.window.request_redraw();
            }
            WindowEvent::RedrawRequested => {
                let frame = match gpu.surface.get_current_texture() {
                    wgpu::CurrentSurfaceTexture::Success(f)
                    | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
                    wgpu::CurrentSurfaceTexture::Outdated
                    | wgpu::CurrentSurfaceTexture::Lost => {
                        gpu.surface.configure(&gpu.device, &gpu.surface_cfg);
                        return;
                    }
                    other => {
                        eprintln!("Surface unavailable: {other:?}");
                        return;
                    }
                };

                let view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                gpu.platform.prepare_frame(&gpu.window, &mut gpu.context);

                let ui = gpu.context.frame();
                gpu.demo.render(ui);
                gpu.platform.prepare_render_with_ui(ui, &gpu.window);

                let draw_data = gpu.context.render();

                let mut encoder = gpu.device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor { label: Some("imgui") },
                );

                {
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("imgui_pass"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.06, g: 0.06, b: 0.08, a: 1.0,
                                }),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });

                    if draw_data.total_vtx_count > 0 {
                        gpu.renderer.render_draw_data(draw_data, &mut pass).expect("render");
                    }
                }

                gpu.queue.submit(Some(encoder.finish()));
                frame.present();
                gpu.window.request_redraw();
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(gpu) = self.gpu.as_ref() {
            gpu.window.request_redraw();
        }
    }
}

fn apply_dark_theme(style: &mut dear_imgui_rs::Style) {
    style.set_window_rounding(6.0);
    style.set_frame_rounding(4.0);
    style.set_grab_rounding(4.0);
    style.set_tab_rounding(4.0);
    style.set_scrollbar_rounding(6.0);
    style.set_window_border_size(1.0);
    style.set_frame_border_size(0.0);
    style.set_popup_rounding(4.0);
    style.set_cell_padding([6.0, 2.0]);
    style.set_frame_padding([3.0, 2.0]);
    style.set_item_spacing([8.0, 4.0]);
    style.set_item_inner_spacing([6.0, 3.0]);

    let accent = [0.40, 0.63, 0.88, 1.0];
    let accent_dim = [0.30, 0.50, 0.75, 1.0];
    let accent_hi = [0.50, 0.73, 0.95, 1.0];

    style.set_color(StyleColor::WindowBg, [0.09, 0.09, 0.11, 1.0]);
    style.set_color(StyleColor::ChildBg, [0.10, 0.10, 0.13, 1.0]);
    style.set_color(StyleColor::PopupBg, [0.11, 0.12, 0.15, 0.96]);
    style.set_color(StyleColor::Border, [0.20, 0.22, 0.27, 0.70]);
    style.set_color(StyleColor::FrameBg, [0.14, 0.15, 0.19, 1.0]);
    style.set_color(StyleColor::FrameBgHovered, [0.19, 0.20, 0.26, 1.0]);
    style.set_color(StyleColor::FrameBgActive, [0.24, 0.26, 0.33, 1.0]);
    style.set_color(StyleColor::TitleBg, [0.09, 0.09, 0.11, 1.0]);
    style.set_color(StyleColor::TitleBgActive, [0.12, 0.13, 0.17, 1.0]);
    style.set_color(StyleColor::ScrollbarBg, [0.08, 0.08, 0.10, 0.60]);
    style.set_color(StyleColor::ScrollbarGrab, [0.22, 0.24, 0.30, 1.0]);
    style.set_color(StyleColor::ScrollbarGrabHovered, [0.30, 0.33, 0.40, 1.0]);
    style.set_color(StyleColor::ScrollbarGrabActive, accent_dim);
    style.set_color(StyleColor::CheckMark, accent);
    style.set_color(StyleColor::SliderGrab, accent_dim);
    style.set_color(StyleColor::SliderGrabActive, accent);
    style.set_color(StyleColor::Button, [0.18, 0.20, 0.25, 1.0]);
    style.set_color(StyleColor::ButtonHovered, [0.26, 0.29, 0.36, 1.0]);
    style.set_color(StyleColor::ButtonActive, accent_dim);
    style.set_color(StyleColor::Header, [0.18, 0.20, 0.25, 1.0]);
    style.set_color(StyleColor::HeaderHovered, [0.24, 0.27, 0.34, 1.0]);
    style.set_color(StyleColor::HeaderActive, accent_dim);
    style.set_color(StyleColor::Separator, [0.20, 0.22, 0.27, 0.60]);
    style.set_color(StyleColor::Tab, [0.14, 0.15, 0.19, 1.0]);
    style.set_color(StyleColor::TabHovered, accent_dim);
    style.set_color(StyleColor::TabSelected, [0.22, 0.24, 0.30, 1.0]);
    style.set_color(StyleColor::TextSelectedBg, [accent[0], accent[1], accent[2], 0.30]);
    style.set_color(StyleColor::Text, [0.92, 0.93, 0.95, 1.0]);
    style.set_color(StyleColor::TextDisabled, [0.42, 0.45, 0.52, 1.0]);
    style.set_color(StyleColor::PlotHistogram, accent_hi);
}

fn main() {
    let event_loop = EventLoop::new().expect("event loop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App::new();
    event_loop.run_app(&mut app).expect("run");
}
