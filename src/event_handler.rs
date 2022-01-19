use crate::config::eventloop::ConfigEvent;
use crate::Anodium;

impl Anodium {
    pub fn process_config_event(&mut self, event: ConfigEvent) {
        match event {
            // TODO: Implement window closing from events
            ConfigEvent::Close(_window) => {}
            ConfigEvent::Maximize(_window) => {
                todo!();
                // self.active_workspace().maximize_request(&window.toplevel());
            }
            ConfigEvent::Unmaximize(_window) => {
                todo!();
                // self.active_workspace()
                //     .unmaximize_request(&window.toplevel());
            }
            ConfigEvent::SwitchWorkspace(_workspace) => todo!(),
            ConfigEvent::OutputsRearrange => {
                self.config
                    .output_rearrange(self.output_manager.outputs().clone());
            }
            ConfigEvent::Shell(fnptr) => {
                self.config.execute_fnptr(fnptr, ());
            }
        }
    }
}
