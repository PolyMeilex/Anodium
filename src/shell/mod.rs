use std::{cell::RefCell, rc::Rc};

use smithay::{
    reexports::wayland_server::Display,
    wayland::shell::wlr_layer::{wlr_layer_shell_init, LayerShellRequest},
};

use crate::state::{Anodium, BackendState};

pub mod move_surface_grab;
pub mod not_mapped_list;
pub mod resize_surface_grab;

pub mod surface_data;
pub use surface_data::SurfaceData;
pub use surface_data::{MoveAfterResizeData, MoveAfterResizeState};
use surface_data::{ResizeEdge, ResizeState};

pub mod shell_manager;

impl Anodium {
    fn wlr_layer_shell_request(&mut self, request: LayerShellRequest) {
        match request {
            LayerShellRequest::NewLayerSurface {
                surface,
                output,
                layer,
                ..
            } => {
                self.desktop_layout
                    .borrow_mut()
                    .insert_layer(output, surface, layer);
            }
            LayerShellRequest::AckConfigure { .. } => {
                self.desktop_layout.borrow_mut().arrange_layers();
            }
        }
    }
}

pub fn init_shell(display: Rc<RefCell<Display>>, log: ::slog::Logger) {
    wlr_layer_shell_init(
        &mut *display.borrow_mut(),
        move |request, mut ddata| {
            let state = ddata.get::<BackendState>().unwrap();
            state.anodium.wlr_layer_shell_request(request);
        },
        log.clone(),
    );
}

// fn fullscreen_output_geometry(
//     wl_surface: &wl_surface::WlSurface,
//     wl_output: Option<&wl_output::WlOutput>,
//     window_map: &WindowMap,
//     output_map: &OutputMap,
// ) -> Option<Rectangle<i32, Logical>> {
//     // First test if a specific output has been requested
//     // if the requested output is not found ignore the request
//     if let Some(wl_output) = wl_output {
//         return output_map.find_by_output(&wl_output).map(|o| o.geometry());
//     }

//     // There is no output preference, try to find the output
//     // where the window is currently active
//     let window_location = window_map
//         .windows()
//         .find(wl_surface)
//         .map(|window| window.location());

//     if let Some(location) = window_location {
//         let window_output = output_map.find_by_position(location).map(|o| o.geometry());

//         if let Some(result) = window_output {
//             return Some(result);
//         }
//     }

//     // Fallback to primary output
//     output_map.with_primary().map(|o| o.geometry())
// }
