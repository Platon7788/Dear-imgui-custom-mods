//! Initial window placement.

use winit::{event_loop::ActiveEventLoop, window::Window};

use super::super::config::Position;

pub(crate) fn position_window(window: &Window, pos: &Position, event_loop: &ActiveEventLoop) {
    match pos {
        Position::Default => { /* OS default */ }
        Position::ScreenCenter => {
            if let Some(mon) = event_loop.primary_monitor() {
                let mp = mon.position();
                let ms = mon.size();
                let ws = window.inner_size();
                // Clamp to i32 before arithmetic — monitor dimensions on
                // multi-monitor / virtual-display setups can exceed 32 767 px
                // (think 4K×4 tiled arrangements), but `i32` covers up to
                // ~2 billion pixels which is well above any real hardware.
                // `saturating_as` avoids the wrapping UB of a bare `as i32`
                // on hypothetical 32-bit hosts where `u32 > i32::MAX`.
                let ms_w = ms.width.min(i32::MAX as u32) as i32;
                let ms_h = ms.height.min(i32::MAX as u32) as i32;
                let ws_w = ws.width.min(i32::MAX as u32) as i32;
                let ws_h = ws.height.min(i32::MAX as u32) as i32;
                window.set_outer_position(winit::dpi::PhysicalPosition::new(
                    mp.x + (ms_w - ws_w) / 2,
                    mp.y + (ms_h - ws_h) / 2,
                ));
            }
        }
        Position::TopLeft => {
            window.set_outer_position(winit::dpi::PhysicalPosition::new(0, 0));
        }
        Position::Custom(x, y) => {
            window.set_outer_position(winit::dpi::PhysicalPosition::new(*x, *y));
        }
    }
}
