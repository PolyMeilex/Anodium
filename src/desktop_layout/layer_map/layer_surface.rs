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

use crate::shell::SurfaceData;

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

        let mut bounding_box = Rectangle::from_loc_and_size(inner.location, (0, 0));
        if let Some(wl_surface) = inner.surface.get_surface() {
            with_surface_tree_downward(
                wl_surface,
                inner.location,
                |_, states, &loc| {
                    let mut loc = loc;
                    let data = states.data_map.get::<RefCell<SurfaceData>>();

                    if let Some(size) = data.and_then(|d| d.borrow().size()) {
                        if states.role == Some("subsurface") {
                            let current = states.cached_state.current::<SubsurfaceCachedState>();
                            loc += current.location;
                        }

                        // Update the bounding box.
                        bounding_box = bounding_box.merge(Rectangle::from_loc_and_size(loc, size));

                        TraversalAction::DoChildren(loc)
                    } else {
                        // If the parent surface is unmapped, then the child surfaces are hidden as
                        // well, no need to consider them here.
                        TraversalAction::SkipChildren
                    }
                },
                |_, _, _| {},
                |_, _, _| true,
            );
        }
        inner.bbox = bounding_box;

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

        if let Some(wl_surface) = inner.surface.get_surface() {
            with_surface_tree_downward(
                wl_surface,
                (),
                |_, _, &()| TraversalAction::DoChildren(()),
                |_, states, &()| {
                    // the surface may not have any user_data if it is a subsurface and has not
                    // yet been commited
                    SurfaceData::send_frame(&mut *states.cached_state.current(), time)
                },
                |_, _, &()| true,
            );
        }
    }
}
