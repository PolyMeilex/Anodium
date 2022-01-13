#![allow(dead_code)]

use smithay::{
    reexports::wayland_server::protocol::wl_surface::{self, WlSurface},
    utils::{Logical, Point, Rectangle},
    wayland::{compositor::with_states, shell::xdg::SurfaceCachedState},
};

use crate::render::{self, renderer::RenderFrame};

mod list;
pub use list::PopupList;

mod popup_surface;
pub use popup_surface::PopupSurface;

#[derive(Debug)]
pub struct Popup {
    pub popup: PopupSurface,
    pub bbox: Rectangle<i32, Logical>,
    pub children: Vec<Popup>,
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
        self.children.push(popup);
    }

    pub fn children(&self) -> &[Popup] {
        &self.children
    }

    /// Sends the frame callback to all the subsurfaces in this
    /// window that requested it
    pub fn send_frame(&self, time: u32) {
        if let Some(surface) = self.popup.get_surface() {
            smithay::desktop::utils::send_frames_surface_tree(surface, time);
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

        for child in self.children.iter() {
            let res = child.matching(parent_location + self.popup.location(), point);
            if res.is_some() {
                return res;
            }
        }

        let mut bbox = self.bbox;
        bbox.loc += parent_location;

        if !bbox.to_f64().contains(point) {
            return None;
        }

        let wl_surface = self.popup.get_surface()?;

        smithay::desktop::utils::under_from_surface_tree(
            wl_surface,
            point,
            // substract geometry
            self.popup.location() + parent_location,
        )
    }

    pub fn self_update(&mut self) {
        if let Some(surface) = self.popup.get_surface() {
            self.bbox =
                smithay::desktop::utils::bbox_from_surface_tree(surface, self.popup.location());
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

    pub fn render(
        &self,
        frame: &mut RenderFrame,
        initial_location: Point<i32, Logical>,
        output_scale: f64,
    ) {
        let render_location = initial_location + self.popup.location();

        if let Some(wl_surface) = self.popup.get_surface() {
            if let Err(err) =
                render::draw_surface_tree(frame, wl_surface, render_location, output_scale)
            {
                error!("{:?}", err);
            }
        }

        for child in self.children.iter() {
            child.render(frame, render_location, output_scale);
        }
    }
}
