use crate::config::eventloop::ConfigEvent;
use crate::positioner::Positioner;
use crate::{window, Anodium};

impl Anodium {
    pub fn process_config_event(&mut self, event: ConfigEvent) {
        match event {
            ConfigEvent::CloseFocused => {}
            ConfigEvent::MaximizeFocused => {
                let under = self.surface_under(self.input_state.pointer_location);
                if let Some((surface, _)) = under {
                    if let Some(space) = self.find_workspace_by_surface_mut(&surface) {
                        if let Some(window) = space.find_window(&surface) {
                            space.maximize_request(&window.toplevel());
                        }
                    }
                }
            }
        }
    }
}
