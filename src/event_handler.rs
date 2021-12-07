use smithay::reexports::calloop::timer::Timer;
use std::time::Duration;

use crate::config::eventloop::ConfigEvent;
use crate::Anodium;

impl Anodium {
    pub fn process_config_event(&mut self, event: ConfigEvent) {
        match event {
            ConfigEvent::CloseFocused => {}
            ConfigEvent::MaximizeFocused => {
                if let Some(window) = self.focused_window.clone() {
                    self.active_workspace().maximize_request(&window.toplevel());
                }
            }
            ConfigEvent::UnmaximizeFocused => {
                if let Some(window) = self.focused_window.clone() {
                    self.active_workspace()
                        .unmaximize_request(&window.toplevel());
                }
            }
            ConfigEvent::SwitchWorkspace(workspace) => self.switch_workspace(&workspace),
            ConfigEvent::Timeout(callback, millis) => {
                let source = Timer::new().expect("Failed to create timer event source!");
                let timer_handle = source.handle();
                timer_handle.add_timeout(Duration::from_millis(millis), (callback, millis));

                self.handle
                    .insert_source(source, move |(callback, millis), _metadata, shared_data| {
                        let callback_cloned = callback.clone();
                        if let Ok(result) = shared_data
                            .config
                            .execute_callback(callback, &mut [])
                            .as_bool()
                        {
                            //rescheduling doesn't work when the callbacks references any outside variables
                            if result {
                                shared_data
                                    .config
                                    .insert_event(ConfigEvent::Timeout(callback_cloned, millis));
                            }
                        }
                    })
                    .unwrap();
            }
        }
    }
}
