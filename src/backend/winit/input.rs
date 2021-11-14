use crate::state::BackendState;

use smithay::{backend::winit::WinitEvent, wayland::output::Mode};

impl BackendState {
    pub fn process_winit_event(&mut self, event: WinitEvent) {
        match event {
            WinitEvent::Resized { size, .. } => {
                self.anodium
                    .desktop_layout
                    .borrow_mut()
                    .update_output_mode_by_name(
                        Mode {
                            size,
                            refresh: 60_000,
                        },
                        crate::backend::winit::OUTPUT_NAME,
                    );
            }
            WinitEvent::Input(event) => {
                self.anodium.process_input_event(event);
            }
            _ => {}
        }
    }
}
