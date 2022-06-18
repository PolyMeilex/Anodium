mod backend_handler;
mod input_handler;
mod output_handler;
// mod shell_handler;

mod compositor;
mod dmabuf;
mod xdg_shell;

//
// Wl Seat
//

use smithay::reexports::wayland_server::protocol::wl_data_source::WlDataSource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler,
};
use smithay::wayland::seat::{SeatHandler, SeatState};
use smithay::{delegate_data_device, delegate_output, delegate_seat};

use crate::State;

impl SeatHandler for State {
    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }
}

delegate_seat!(State);

//
// Wl Data Device
//

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
        _seat: smithay::wayland::seat::Seat<Self>,
    ) {
        self.pointer_icon.dnd_started(icon);
    }

    fn dropped(&mut self, _seat: smithay::wayland::seat::Seat<Self>) {
        self.pointer_icon.dnd_dropped();
    }
}
impl ServerDndGrabHandler for State {}

delegate_data_device!(State);

//
// Wl Output & Xdg Output
//

delegate_output!(State);
