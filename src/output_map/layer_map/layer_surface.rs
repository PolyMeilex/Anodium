use smithay::{
    reexports::wayland_server::protocol::wl_surface,
    utils::{Logical, Point, Rectangle},
    wayland::{
        compositor::with_states,
        shell::wlr_layer::{self, LayerSurfaceCachedState},
    },
};

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug)]
struct Inner {
    pub location: Point<i32, Logical>,
    pub bbox: Rectangle<i32, Logical>,

    pub surface: wlr_layer::LayerSurface,
    pub layer: wlr_layer::Layer,
}

#[derive(Debug, Clone)]
pub struct LayerSurface {
    inner: Rc<RefCell<Inner>>,
}

impl LayerSurface {
    pub fn location(&self) -> Point<i32, Logical> {
        self.inner.borrow().location
    }
    pub fn set_location(&mut self, location: Point<i32, Logical>) {
        self.inner.borrow_mut().location = location;
    }

    pub fn bbox(&self) -> Rectangle<i32, Logical> {
        self.inner.borrow().bbox
    }

    pub fn surface(&self) -> wlr_layer::LayerSurface {
        self.inner.borrow().surface.clone()
    }

    pub fn layer(&self) -> wlr_layer::Layer {
        self.inner.borrow().layer
    }
}

impl LayerSurface {
    pub fn new(surface: wlr_layer::LayerSurface, layer: wlr_layer::Layer) -> Self {
        Self {
            inner: Rc::new(RefCell::new(Inner {
                location: Default::default(),
                bbox: Default::default(),

                surface,
                layer,
            })),
        }
    }

    /// Finds the topmost surface under this point if any and returns it together with the location of this
    /// surface.
    pub fn matching(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(wl_surface::WlSurface, Point<i32, Logical>)> {
        let inner = &self.inner.borrow();

        if !inner.bbox.to_f64().contains(point) {
            return None;
        }

        let wl_surface = inner.surface.get_surface()?;
        smithay::desktop::utils::under_from_surface_tree(wl_surface, point, inner.location)
    }

    pub fn self_update(&mut self) {
        let inner = &mut self.inner.borrow_mut();

        if let Some(surface) = inner.surface.get_surface() {
            inner.bbox = smithay::desktop::utils::bbox_from_surface_tree(surface, inner.location);
        }

        if let Some(surface) = inner.surface.get_surface() {
            inner.layer = with_states(surface, |states| {
                let current = states.cached_state.current::<LayerSurfaceCachedState>();
                current.layer
            })
            .unwrap();
        }
    }

    /// Sends the frame callback to all the subsurfaces in this
    /// window that requested it
    pub fn send_frame(&self, time: u32) {
        let inner = &self.inner.borrow();

        if let Some(surface) = inner.surface.get_surface() {
            smithay::desktop::utils::send_frames_surface_tree(surface, time);
        }
    }
}
