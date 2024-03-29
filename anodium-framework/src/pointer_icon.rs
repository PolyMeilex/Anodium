use std::sync::{Arc, Mutex};

use smithay::{
    desktop::space::SurfaceTree,
    input::pointer::CursorImageStatus,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{IsAlive, Logical, Point},
};

#[derive(Debug, Clone)]
pub struct PointerIcon {
    dnd_surface: Arc<Mutex<Option<WlSurface>>>,
    pointer_icon: Arc<Mutex<CursorImageStatus>>,
}

impl Default for PointerIcon {
    fn default() -> Self {
        Self {
            dnd_surface: Arc::new(Mutex::new(None)),
            pointer_icon: Arc::new(Mutex::new(CursorImageStatus::Default)),
        }
    }
}

impl PointerIcon {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn dnd_started(&self, icon: Option<WlSurface>) {
        *self.dnd_surface.lock().unwrap() = icon;
    }

    pub fn dnd_dropped(&self) {
        *self.dnd_surface.lock().unwrap() = None;
    }

    pub fn on_new_cursor(&self, status: CursorImageStatus) {
        *self.pointer_icon.lock().unwrap() = status;
    }

    pub fn prepare_dnd_icon(&self, location: Point<i32, Logical>) -> Option<SurfaceTree> {
        if let Some(surface) = &*self.dnd_surface.lock().unwrap() {
            surface
                .alive()
                .then(|| crate::draw::draw_dnd_icon(surface.clone(), location))
        } else {
            None
        }
    }

    pub fn prepare_cursor_icon(&self, location: Point<i32, Logical>) -> Option<SurfaceTree> {
        let mut cursor_status = self.pointer_icon.lock().unwrap();

        // reset the cursor if the surface is no longer alive
        let reset = if let CursorImageStatus::Surface(ref surface) = *cursor_status {
            !surface.alive()
        } else {
            false
        };

        if reset {
            *cursor_status = CursorImageStatus::Default;
        }

        if let CursorImageStatus::Surface(ref wl_surface) = *cursor_status {
            Some(crate::draw::draw_cursor(wl_surface.clone(), location))
        } else {
            None
        }
    }
}
