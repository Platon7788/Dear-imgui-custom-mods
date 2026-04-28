//! Initial window placement.

use winit::{event_loop::ActiveEventLoop, window::Window};

use super::super::config::PositionV2;

pub(crate) fn position_window(window: &Window, pos: &PositionV2, event_loop: &ActiveEventLoop) {
    match pos {
        PositionV2::Default => { /* OS default */ }
        PositionV2::ScreenCenter => {
            if let Some(mon) = event_loop.primary_monitor() {
                let mp = mon.position();
                let ms = mon.size();
                let ws = window.inner_size();
                window.set_outer_position(winit::dpi::PhysicalPosition::new(
                    mp.x + (ms.width as i32 - ws.width as i32) / 2,
                    mp.y + (ms.height as i32 - ws.height as i32) / 2,
                ));
            }
        }
        PositionV2::TopLeft => {
            window.set_outer_position(winit::dpi::PhysicalPosition::new(0, 0));
        }
        PositionV2::Custom(x, y) => {
            window.set_outer_position(winit::dpi::PhysicalPosition::new(*x, *y));
        }
    }
}
