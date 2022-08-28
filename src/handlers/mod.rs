mod backend_handler;
mod input_handler;
mod output_handler;

mod compositor;
mod dmabuf;
mod xdg_shell;

//
// Wl Seat
//

use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_server::protocol::wl_data_source::WlDataSource;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource;
use smithay::wayland::data_device::{
    self, ClientDndGrabHandler, DataDeviceHandler, ServerDndGrabHandler,
};
use smithay::{delegate_data_device, delegate_output, delegate_seat};

use crate::State;

impl SeatHandler for State {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&Self::KeyboardFocus>) {
        let focus = focused.and_then(|s| self.display.get_client(s.id()).ok());
        data_device::set_data_device_focus(&self.display, seat, focus);
    }

    fn cursor_image(
        &mut self,
        _seat: &Seat<Self>,
        image: smithay::input::pointer::CursorImageStatus,
    ) {
        self.pointer_icon.on_new_cursor(image);
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

//
// Wl Output & Xdg Output
//

delegate_output!(State);
