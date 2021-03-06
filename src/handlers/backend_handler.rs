use crate::{CalloopData, State};
use anodium_backend::{BackendHandler, BackendState};

impl BackendHandler for CalloopData {
    type WaylandState = State;

    fn backend_state(&mut self) -> &mut BackendState {
        &mut self.state.backend
    }

    fn send_frames(&mut self) {
        self.state
            .space
            .send_frames(self.state.start_time.elapsed().as_millis() as u32);
    }

    fn start_compositor(&mut self) {
        ::std::env::set_var("WAYLAND_DISPLAY", &self.state.socket_name);
        dbg!(&self.state.socket_name);

        #[cfg(feature = "xwayland")]
        {
            self.state
                .xwayland
                .start::<CalloopData>(self.state.loop_handle.clone())
                .ok();
        }
    }

    fn close_compositor(&mut self) {
        self.state.loop_signal.stop();
    }
}
