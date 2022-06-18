use smithay::{
    desktop::{Kind, PopupKind},
    wayland::{
        seat::{PointerGrabStartData, Seat},
        shell::xdg::XdgRequest,
        Serial,
    },
};

use super::ShellHandler;

use super::utils::AsWlSurface;

impl<D> super::Inner<D>
where
    D: ShellHandler,
{
    pub fn xdg_shell_request(&mut self, request: XdgRequest, handler: &mut D) {
        match request {
            //
            // Toplevel
            //
            XdgRequest::NewToplevel { surface } => {
                self.not_mapped_list.insert_window(Kind::Xdg(surface), None);
            }

            XdgRequest::Move {
                seat,
                serial,
                surface,
            } => {
                let seat = Seat::from_resource(&seat).unwrap();

                if let Some(start_data) = check_grab(&seat, serial, &surface) {
                    let window = self.windows.find(&surface);

                    if let Some(window) = window.cloned() {
                        handler.window_move(window, start_data, seat, serial);
                    }
                }
            }
            XdgRequest::Resize {
                surface,
                seat,
                serial,
                edges,
            } => {
                let seat = Seat::from_resource(&seat).unwrap();

                if let Some(start_data) = check_grab(&seat, serial, &surface) {
                    let window = self.windows.find(&surface);

                    if let Some(window) = window.cloned() {
                        handler.window_resize(window, start_data, seat, edges.into(), serial);
                    }
                }
            }

            XdgRequest::Maximize { surface } => {
                let window = self.windows.find(&surface);
                if let Some(window) = window.cloned() {
                    handler.window_maximize(window);
                }
            }
            XdgRequest::UnMaximize { surface } => {
                let window = self.windows.find(&surface);
                if let Some(window) = window.cloned() {
                    handler.window_unmaximize(window);
                }
            }

            XdgRequest::Fullscreen { surface, output } => {
                let window = self.windows.find(&surface);
                if let Some(window) = window.cloned() {
                    handler.window_fullscreen(window, output);
                }
            }
            XdgRequest::UnFullscreen { surface } => {
                let window = self.windows.find(&surface);
                if let Some(window) = window.cloned() {
                    handler.window_unfullscreen(window);
                }
            }

            XdgRequest::Minimize { surface } => {
                let window = self.windows.find(&surface);
                if let Some(window) = window.cloned() {
                    handler.window_minimize(window);
                }
            }

            //
            // Popup
            //
            XdgRequest::NewPopup { surface, .. } => {
                self.popup_manager
                    .track_popup(PopupKind::Xdg(surface))
                    .unwrap();
            }
            XdgRequest::Grab {
                seat,
                serial,
                surface,
            } => {
                let seat = Seat::from_resource(&seat).unwrap();

                if let Some(start_data) = check_grab(&seat, serial, &surface) {
                    handler.popup_grab(PopupKind::Xdg(surface), start_data, seat, serial);
                }
            }
            XdgRequest::RePosition { .. } => {
                error!("TODO: Popup RePosition");
            }

            //
            // Misc
            //
            XdgRequest::ShowWindowMenu {
                surface,
                seat,
                serial,
                location,
            } => {
                let window = self.windows.find(&surface);
                if let Some(window) = window.cloned() {
                    handler.show_window_menu(
                        window,
                        Seat::from_resource(&seat).unwrap(),
                        serial,
                        location,
                    );
                }
            }
            _ => {}
        }
    }
}

fn check_grab<S: AsWlSurface>(
    seat: &Seat,
    serial: Serial,
    surface: &S,
) -> Option<PointerGrabStartData> {
    let surface = surface.as_surface()?;
    let pointer = seat.get_pointer()?;

    // Check that this surface has a click grab.
    if pointer.has_grab(serial) {
        let start_data = pointer.grab_start_data()?;
        let focus = start_data.focus.as_ref()?;

        if focus.0.as_ref().same_client_as(surface.as_ref()) {
            Some(start_data)
        } else {
            // If the focus was for a different surface, ignore the request.
            None
        }
    } else {
        None
    }
}
