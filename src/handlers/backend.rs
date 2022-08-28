use anodium_backend::{BackendHandler, BackendState};
use smithay::wayland::dmabuf::DmabufState;

use crate::{CalloopData, State};

impl BackendHandler for CalloopData {
    type WaylandState = State;

    fn backend_state(&mut self) -> &mut BackendState {
        &mut self.state.backend
    }

    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.state.dmabuf_state
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
