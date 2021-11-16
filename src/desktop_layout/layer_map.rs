use smithay::{
    reexports::wayland_server::protocol::wl_surface,
    utils::{Logical, Point, Rectangle},
    wayland::{
        compositor::with_states,
        shell::wlr_layer::{self, Anchor, ExclusiveZone, LayerSurfaceCachedState},
    },
};

mod layer_surface;
pub use layer_surface::LayerSurface;

#[derive(Default, Debug)]
pub struct LayerExclusiveZone {
    pub top: u32,
    pub bottom: u32,
    pub left: u32,
    pub right: u32,
}

#[derive(Default, Debug)]
pub struct LayerMap {
    surfaces: Vec<LayerSurface>,
    exclusive_zone: LayerExclusiveZone,
}

impl LayerMap {
    pub fn exclusive_zone(&self) -> &LayerExclusiveZone {
        &self.exclusive_zone
    }
}

impl LayerMap {
    pub fn insert(&mut self, mut layer: LayerSurface) {
        layer.self_update();
        self.surfaces.insert(0, layer);
    }

    pub fn get_surface_under(
        &self,
        layer: &wlr_layer::Layer,
        point: Point<f64, Logical>,
    ) -> Option<(wl_surface::WlSurface, Point<i32, Logical>)> {
        for l in self.surfaces.iter().filter(|s| &s.layer() == layer) {
            if let Some(surface) = l.matching(point) {
                return Some(surface);
            }
        }
        None
    }

    pub fn with_layers_from_bottom_to_top<Func>(&self, layer: &wlr_layer::Layer, mut f: Func)
    where
        Func: FnMut(&LayerSurface),
    {
        for l in self.surfaces.iter().filter(|s| &s.layer() == layer).rev() {
            f(l)
        }
    }

    pub fn refresh(&mut self) {
        self.surfaces.retain(|l| l.surface().alive());

        // Note: Already updated in ShellManager::refresh
        // for l in self.surfaces.iter_mut() {
        //     l.self_update();
        // }
    }

    #[allow(dead_code)]
    /// Finds the layer corresponding to the given `WlSurface`.
    pub fn find(&self, surface: &wl_surface::WlSurface) -> Option<&LayerSurface> {
        self.surfaces.iter().find_map(|l| {
            if l.surface()
                .get_surface()
                .map(|s| s.as_ref().equals(surface.as_ref()))
                .unwrap_or(false)
            {
                Some(l)
            } else {
                None
            }
        })
    }

    pub fn arange(&mut self, output_rect: Rectangle<i32, Logical>) {
        self.exclusive_zone = Default::default();

        for layer in self.surfaces.iter_mut() {
            let surface = layer.surface();
            let surface = if let Some(surface) = surface.get_surface() {
                surface
            } else {
                continue;
            };

            let data = with_states(surface, |states| {
                *states.cached_state.current::<LayerSurfaceCachedState>()
            })
            .unwrap();

            let x = if data.size.w == 0 || data.anchor.contains(Anchor::LEFT) {
                output_rect.loc.x
            } else if data.anchor.contains(Anchor::RIGHT) {
                output_rect.loc.x + (output_rect.size.w - data.size.w)
            } else {
                output_rect.loc.x + ((output_rect.size.w / 2) - (data.size.w / 2))
            };

            let y = if data.size.h == 0 || data.anchor.contains(Anchor::TOP) {
                output_rect.loc.y
            } else if data.anchor.contains(Anchor::BOTTOM) {
                output_rect.loc.y + (output_rect.size.h - data.size.h)
            } else {
                output_rect.loc.y + ((output_rect.size.h / 2) - (data.size.h / 2))
            };

            let location: Point<i32, Logical> = (x, y).into();

            layer
                .surface()
                .with_pending_state(|state| {
                    state.size = Some(output_rect.size);
                })
                .unwrap();

            layer.surface().send_configure();

            layer.set_location(location);

            if let ExclusiveZone::Exclusive(v) = data.exclusive_zone {
                let anchor = data.anchor;

                // Top
                if anchor == (Anchor::TOP) {
                    self.exclusive_zone.top += v;
                }
                if anchor == (Anchor::TOP | Anchor::LEFT | Anchor::RIGHT) {
                    self.exclusive_zone.top += v;
                }

                // Bottom
                if anchor == (Anchor::BOTTOM) {
                    self.exclusive_zone.bottom += v;
                }
                if anchor == (Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT) {
                    self.exclusive_zone.bottom += v;
                }

                // Left
                if anchor == (Anchor::LEFT) {
                    self.exclusive_zone.left += v;
                }
                if anchor == (Anchor::LEFT | Anchor::BOTTOM | Anchor::TOP) {
                    self.exclusive_zone.left += v;
                }

                // Right
                if anchor == (Anchor::RIGHT) {
                    self.exclusive_zone.right += v;
                }
                if anchor == (Anchor::RIGHT | Anchor::BOTTOM | Anchor::TOP) {
                    self.exclusive_zone.right += v;
                }
            }
        }
    }

    pub fn send_frames(&self, time: u32) {
        for layer in &self.surfaces {
            layer.send_frame(time);
        }
    }
}
