use crate::{desktop_layout::LayerSurface, utils::AsWlSurface};

#[derive(Debug, Default)]
pub struct ShellLayerList {
    layers: Vec<LayerSurface>,
}

impl ShellLayerList {
    pub fn push(&mut self, window: LayerSurface) {
        self.layers.push(window)
    }

    pub fn find<S: AsWlSurface>(&self, surface: &S) -> Option<&LayerSurface> {
        surface.as_surface().and_then(|surface| {
            self.layers.iter().find_map(|w| {
                if w.surface()
                    .get_surface()
                    .map(|s| s.as_ref().equals(surface.as_ref()))
                    .unwrap_or(false)
                {
                    Some(w)
                } else {
                    None
                }
            })
        })
    }

    /// Finds the toplevel corresponding to the given `WlSurface`.
    // pub fn find_mut<S: AsWlSurface>(&mut self, surface: &S) -> Option<&mut LayerSurface> {
    //     if let Some(surface) = surface.as_surface() {
    //         self.layers.iter_mut().find_map(|w| {
    //             if w.surface()
    //                 .get_surface()
    //                 .map(|s| s.as_ref().equals(surface.as_ref()))
    //                 .unwrap_or(false)
    //             {
    //                 Some(w)
    //             } else {
    //                 None
    //             }
    //         })
    //     } else {
    //         None
    //     }
    // }

    pub fn refresh(&mut self) {
        self.layers.retain(|l| l.surface().alive());

        for l in self.layers.iter_mut() {
            l.self_update();
        }
    }
}
