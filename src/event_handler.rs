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
            ConfigEvent::Timeout(fnptr, millis) => {
                let source = Timer::new().expect("Failed to create timer event source!");
                let timer_handle = source.handle();
                timer_handle.add_timeout(Duration::from_millis(millis), (fnptr, millis));

                self.handle
                    .insert_source(source, move |(fnptr, millis), _metadata, shared_data| {
                        let fnptr_cloned = fnptr.clone();
                        if let Ok(result) = shared_data.config.execute_fnptr(fnptr, ()).as_bool() {
                            if result {
                                shared_data
                                    .config
                                    .insert_event(ConfigEvent::Timeout(fnptr_cloned, millis));
                            }
                        }
                    })
                    .unwrap();
            }
            ConfigEvent::OutputsRearrange => {
                self.config.output_rearrange();
            }
        }
    }
}
