#![allow(dead_code)]

use std::{cell::RefCell, sync::Mutex};

use smithay::{
    reexports::wayland_server::protocol::wl_surface,
    utils::{Logical, Point, Rectangle},
    wayland::{
        compositor::{
            with_states, with_surface_tree_downward, SubsurfaceCachedState, TraversalAction,
        },
        shell::xdg::{PopupSurface, SurfaceCachedState, XdgPopupSurfaceRoleAttributes},
    },
};

use crate::{shell::SurfaceData, utils};

mod list;
pub use list::PopupList;

#[derive(Clone, PartialEq)]
pub enum PopupKind {
    Xdg(PopupSurface),
}

impl PopupKind {
    pub fn alive(&self) -> bool {
        match *self {
            PopupKind::Xdg(ref t) => t.alive(),
        }
    }

    pub fn get_surface(&self) -> Option<&wl_surface::WlSurface> {
        match *self {
            PopupKind::Xdg(ref t) => t.get_surface(),
        }
    }

    pub fn parent(&self) -> Option<wl_surface::WlSurface> {
        let wl_surface = match self.get_surface() {
            Some(s) => s,
            None => return None,
        };
        with_states(wl_surface, |states| {
            states
                .data_map
                .get::<Mutex<XdgPopupSurfaceRoleAttributes>>()
                .unwrap()
                .lock()
                .unwrap()
                .parent
                .clone()
        })
        .ok()
        .flatten()
    }

    pub fn geometry(&self) -> Rectangle<i32, Logical> {
        let wl_surface = match self.get_surface() {
            Some(s) => s,
            None => return Default::default(),
        };
        with_states(wl_surface, |states| {
            states
                .data_map
                .get::<Mutex<XdgPopupSurfaceRoleAttributes>>()
                .unwrap()
                .lock()
                .unwrap()
                .current
                .geometry
        })
        .unwrap_or_default()
    }

    pub fn location(&self) -> Point<i32, Logical> {
        self.geometry().loc
    }
}

#[derive(Clone)]
pub struct Popup {
    pub popup: PopupKind,
    pub bbox: Rectangle<i32, Logical>,
}

impl Popup {
    /// Sends the frame callback to all the subsurfaces in this
    /// window that requested it
    pub fn send_frame(&self, time: u32) {
        if let Some(surface) = self.popup.get_surface() {
            utils::surface_send_frame(surface, time)
        }
    }

    /// Finds the topmost surface under this point if any and returns it together with the location of this
    /// surface.
    pub fn matching(
        &self,
        parent_location: Point<i32, Logical>,
        point: Point<f64, Logical>,
    ) -> Option<(wl_surface::WlSurface, Point<i32, Logical>)> {
        if !self.popup.alive() {
            return None;
        }

        let mut bbox = self.bbox;
        bbox.loc += parent_location;

        if !bbox.to_f64().contains(point) {
            return None;
        }
        // need to check more carefully
        let found = RefCell::new(None);
        if let Some(wl_surface) = self.popup.get_surface() {
            with_surface_tree_downward(
                wl_surface,
                self.popup.location() + parent_location,
                |wl_surface, states, location| {
                    let mut location = *location;
                    let data = states.data_map.get::<RefCell<SurfaceData>>();

                    if states.role == Some("subsurface") {
                        let current = states.cached_state.current::<SubsurfaceCachedState>();
                        location += current.location;
                    }

                    let contains_the_point = data
                        .map(|data| {
                            data.borrow().contains_point(
                                &*states.cached_state.current(),
                                point - location.to_f64(),
                            )
                        })
                        .unwrap_or(false);
                    if contains_the_point {
                        *found.borrow_mut() = Some((wl_surface.clone(), location));
                    }

                    TraversalAction::DoChildren(location)
                },
                |_, _, _| {},
                |_, _, _| {
                    // only continue if the point is not found
                    found.borrow().is_none()
                },
            );
        }
        found.into_inner()
    }

    pub fn self_update(&mut self) {
        if let Some(surface) = self.popup.get_surface() {
            let mut bbox = utils::surface_bounding_box(surface);
            bbox.loc += self.popup.location();
            self.bbox = bbox;
        }
    }

    /// Returns the geometry of this window.
    #[allow(dead_code)]
    pub fn geometry(&self) -> Rectangle<i32, Logical> {
        // It's the set geometry with the full bounding box as the fallback.
        with_states(self.popup.get_surface().unwrap(), |states| {
            states.cached_state.current::<SurfaceCachedState>().geometry
        })
        .unwrap()
        .unwrap_or(self.bbox)
    }
}
