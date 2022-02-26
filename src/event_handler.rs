use crate::config::eventloop::ConfigEvent;
use crate::Anodium;
use anodium_backend::BackendRequest;

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
                self.config.output_rearrange();
            }
            ConfigEvent::OutputUpdateMode(output, mode) => {
                let output: &smithay::wayland::output::Output = &output;
                self.backend_tx
                    .send(BackendRequest::UpdateMode(output.clone(), mode))
                    .unwrap();
            }
            ConfigEvent::Shell(fnptr) => {
                self.config.execute_fnptr(fnptr, ());
            }
        }
    }
}
