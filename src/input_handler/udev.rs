use crate::{backend::udev::UdevData, state::BackendState};

use smithay::backend::input::{InputBackend, InputEvent};

impl BackendState<UdevData> {
    pub fn process_input_event<B>(&mut self, event: InputEvent<B>)
    where
        B: InputBackend,
    {
        match event {
            event => {
                self.main_state.process_input_event(&mut self.backend_data, event);
            }
        }
    }
}
