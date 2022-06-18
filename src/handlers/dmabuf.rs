use smithay::{
    backend::allocator::dmabuf::Dmabuf,
    delegate_dmabuf,
    reexports::wayland_server::DisplayHandle,
    wayland::dmabuf::{DmabufGlobal, DmabufHandler, DmabufState, ImportError},
};

use crate::{CalloopData, State};

impl DmabufHandler for State {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(
        &mut self,
        dh: &DisplayHandle,
        global: &DmabufGlobal,
        dmabuf: Dmabuf,
    ) -> Result<(), ImportError> {
        self.backend.dmabuf_imported(dh, global, dmabuf)
    }
}

impl AsMut<DmabufState> for CalloopData {
    fn as_mut(&mut self) -> &mut DmabufState {
        self.state.dmabuf_state()
    }
}

delegate_dmabuf!(State);
