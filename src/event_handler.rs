use smithay::reexports::calloop::timer::Timer;
use std::time::Duration;

use crate::config::eventloop::ConfigEvent;
use crate::Anodium;

impl Anodium {
    pub fn process_config_event(&mut self, event: ConfigEvent) {
        match event {
            // TODO: Implement window closing from events
            ConfigEvent::Close(_window) => {}
            ConfigEvent::Maximize(window) => {
                self.active_workspace().maximize_request(&window.toplevel());
            }
            ConfigEvent::Unmaximize(window) => {
                self.active_workspace()
                    .unmaximize_request(&window.toplevel());
            }
            ConfigEvent::SwitchWorkspace(workspace) => self.switch_workspace(&workspace),
            ConfigEvent::OutputsRearrange => {
                self.config.output_rearrange();
            }
        }
    }
}
