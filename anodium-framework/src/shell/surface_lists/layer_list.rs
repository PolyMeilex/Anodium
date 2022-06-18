use super::super::utils::AsWlSurface;
use smithay::desktop::LayerSurface;

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
            self.layers.iter().find(|w| {
                w.layer_surface()
                    .get_surface()
                    .map(|s| s.as_ref().equals(surface.as_ref()))
                    .unwrap_or(false)
            })
        })
    }

    pub fn refresh(&mut self) {
        self.layers.retain(|l| l.layer_surface().alive());
    }
}
