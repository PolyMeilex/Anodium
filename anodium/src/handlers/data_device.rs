use smithay::{
    delegate_data_device,
    input::Seat,
    reexports::wayland_server::protocol::{wl_data_source::WlDataSource, wl_surface::WlSurface},
    wayland::data_device::{ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler},
};

use crate::State;

impl DataDeviceHandler for State {
    fn data_device_state(&self) -> &smithay::wayland::data_device::DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for State {
    fn started(
        &mut self,
        _source: Option<WlDataSource>,
        icon: Option<WlSurface>,
        _seat: Seat<Self>,
    ) {
        self.pointer_icon.dnd_started(icon);
    }

    fn dropped(&mut self, _seat: Seat<Self>) {
        self.pointer_icon.dnd_dropped();
    }
}
impl ServerDndGrabHandler for State {}

delegate_data_device!(State);
