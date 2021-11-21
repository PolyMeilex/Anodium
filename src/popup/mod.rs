#![allow(dead_code)]

use std::cell::RefCell;

use smithay::{
    reexports::wayland_server::protocol::wl_surface::{self, WlSurface},
    utils::{Logical, Point, Rectangle},
    wayland::{
        compositor::{
            with_states, with_surface_tree_downward, SubsurfaceCachedState, TraversalAction,
        },
        shell::xdg::SurfaceCachedState,
    },
};

use crate::{framework::surface_data::SurfaceData, utils};

mod list;
pub use list::PopupList;

mod popup_surface;
pub use popup_surface::PopupSurface;

#[derive(Debug)]
pub struct Popup {
    pub popup: PopupSurface,
    pub bbox: Rectangle<i32, Logical>,
    pub children: Vec<Box<Popup>>,
}

impl Popup {
    pub fn popup_surface(&self) -> PopupSurface {
        self.popup.clone()
    }

    // Try to find popup in tree including self
    pub fn find_popup_in_tree(&mut self, surface: &WlSurface) -> Option<&mut Popup> {
        // Check self
        if self.popup_surface().get_surface() == Some(surface) {
            Some(self)
        } else {
            self.children
                .iter_mut()
                .find_map(|popup| popup.find_popup_in_tree(surface))
        }
    }

    pub fn add_child(&mut self, popup: Popup) {
        self.children.push(Box::new(popup));
    }

    pub fn children(&self) -> &[Box<Popup>] {
        &self.children
    }

    /// Sends the frame callback to all the subsurfaces in this
    /// window that requested it
    pub fn send_frame(&self, time: u32) {
        if let Some(surface) = self.popup.get_surface() {
            utils::surface_send_frame(surface, time)
        }

        for child in self.children.iter() {
            child.send_frame(time);
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
        let wl_surface = self.popup.get_surface()?;

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

        found.into_inner()
    }

    pub fn self_update(&mut self) {
        if let Some(surface) = self.popup.get_surface() {
            let mut bbox = utils::surface_bounding_box(surface);
            bbox.loc += self.popup.location();
            self.bbox = bbox;
        }

        for child in self.children.iter_mut() {
            child.self_update();
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
