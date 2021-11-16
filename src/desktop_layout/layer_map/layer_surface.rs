use smithay::{
    reexports::wayland_server::protocol::wl_surface,
    utils::{Logical, Point, Rectangle},
    wayland::{
        compositor::{
            with_states, with_surface_tree_downward, SubsurfaceCachedState, TraversalAction,
        },
        shell::wlr_layer::{self, LayerSurfaceCachedState},
    },
};

use std::cell::RefCell;
use std::rc::Rc;

use crate::{shell::SurfaceData, utils};

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
        self.inner.borrow().layer.clone()
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
        // need to check more carefully
        let found = RefCell::new(None);
        if let Some(wl_surface) = inner.surface.get_surface() {
            with_surface_tree_downward(
                wl_surface,
                inner.location,
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
        let inner = &mut self.inner.borrow_mut();

        if let Some(surface) = inner.surface.get_surface() {
            let mut bbox = utils::surface_bounding_box(surface);
            bbox.loc += inner.location;
            inner.bbox = bbox;
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
            utils::surface_send_frame(&surface, time)
        }
    }
}
