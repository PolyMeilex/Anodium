use slog_scope::{debug, error};
use smithay::{
    backend::renderer::utils::RendererSurfaceStateUserData,
    desktop::{Kind, Window, X11Surface},
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
    wayland::compositor,
};

use x11rb::protocol::xproto::Window as X11Window;

#[derive(Debug)]
pub struct PendingWindow {
    pub window: Window,
    _location: Point<i32, Logical>,
}

impl PendingWindow {
    pub fn new(
        window: X11Window,
        surface: WlSurface,
        location: Point<i32, Logical>,
    ) -> Option<Self> {
        debug!("Matched X11 surface {:x?} to {:x?}", window, surface);

        if compositor::give_role(&surface, "x11_surface").is_err() {
            // It makes no sense to post a protocol error here since that would only kill Xwayland
            error!("Surface {:x?} already has a role?!", surface);
            return None;
        }

        let x11surface = X11Surface { surface };
        let window = Window::new(Kind::X11(x11surface));

        Some(Self {
            window,
            _location: location,
        })
    }

    pub fn is_buffer_attached(&self) -> bool {
        // Panic free version of `with_renderer_surface_state`
        compositor::with_states(self.window.toplevel().wl_surface(), |states| {
            if let Some(data) = states.data_map.get::<RendererSurfaceStateUserData>() {
                data.borrow().wl_buffer().is_some()
            } else {
                false
            }
        })
    }
}
