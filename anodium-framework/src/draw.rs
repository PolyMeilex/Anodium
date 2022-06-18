use std::sync::Mutex;

use smithay::{
    desktop::space::SurfaceTree,
    reexports::wayland_server::protocol::wl_surface,
    utils::{Logical, Point},
    wayland::{compositor, seat::CursorImageAttributes},
};

pub fn draw_cursor(
    surface: wl_surface::WlSurface,
    location: impl Into<Point<i32, Logical>>,
) -> SurfaceTree {
    let mut position = location.into();
    let ret = compositor::with_states(&surface, |states| {
        Some(
            states
                .data_map
                .get::<Mutex<CursorImageAttributes>>()
                .unwrap()
                .lock()
                .unwrap()
                .hotspot,
        )
    });

    position -= match ret {
        Some(h) => h,
        None => {
            warn!(
                "Trying to display as a cursor a surface that does not have the CursorImage role."
            );
            (0, 0).into()
        }
    };
    SurfaceTree {
        surface,
        position,
        z_index: 100,
    }
}

pub fn draw_dnd_icon(surface: wl_surface::WlSurface, position: Point<i32, Logical>) -> SurfaceTree {
    if compositor::get_role(&surface) != Some("dnd_icon") {
        warn!("Trying to display as a dnd icon a surface that does not have the DndIcon role.");
    }
    SurfaceTree {
        surface,
        position,
        z_index: 100,
    }
}
