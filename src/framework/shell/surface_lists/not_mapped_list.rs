use std::sync::Mutex;

use smithay::desktop::Kind;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor;
use smithay::wayland::shell::xdg::XdgToplevelSurfaceRoleAttributes;

use crate::utils::AsWlSurface;

use smithay::desktop::Window;

#[derive(Default)]
pub struct NotMappedList {
    windows: Vec<Window>,
}

/// Toplevel Windows
impl NotMappedList {
    pub fn insert_window(&mut self, toplevel: Kind) {
        let window = Window::new(toplevel);
        window.refresh();
        self.windows.push(window);
    }

    pub fn find_window_mut<S: AsWlSurface>(&mut self, surface: &S) -> Option<&mut Window> {
        if let Some(surface) = surface.as_surface() {
            self.windows.iter_mut().find_map(|win| {
                if win
                    .toplevel()
                    .get_surface()
                    .map(|s| s.as_ref().equals(surface.as_ref()))
                    .unwrap_or(false)
                {
                    Some(win)
                } else {
                    None
                }
            })
        } else {
            None
        }
    }

    pub fn try_window_map(&mut self, surface: &WlSurface) -> Option<Window> {
        let toplevel = self.find_window_mut(surface).and_then(|win| {
            let toplevel = win.toplevel();
            // send the initial configure if relevant
            if let Kind::Xdg(ref toplevel) = toplevel {
                let initial_configure_sent = compositor::with_states(surface, |states| {
                    states
                        .data_map
                        .get::<Mutex<XdgToplevelSurfaceRoleAttributes>>()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .initial_configure_sent
                })
                .unwrap();
                if !initial_configure_sent {
                    toplevel.send_configure();
                }
            }

            // TODO: this is not true
            let has_buffer = true;
            // let has_buffer =
            //     SurfaceData::try_with(surface, |data| data.buffer.is_some()).unwrap_or(false);

            if has_buffer {
                match toplevel {
                    Kind::Xdg(_) => {
                        let configured = compositor::with_states(surface, |states| {
                            states
                                .data_map
                                .get::<Mutex<XdgToplevelSurfaceRoleAttributes>>()
                                .unwrap()
                                .lock()
                                .unwrap()
                                .configured
                        })
                        .unwrap();

                        if configured {
                            Some(toplevel)
                        } else {
                            None
                        }
                    }
                    #[cfg(feature = "xwayland")]
                    WindowSurface::X11(_) => Some(toplevel),
                }
            } else {
                None
            }
        });

        toplevel
            .cloned()
            .and_then(|toplevel| self.remove_window(&toplevel))
    }

    pub fn remove_window(&mut self, kind: &Kind) -> Option<Window> {
        let id = self.windows.iter().enumerate().find_map(|(id, win)| {
            if win.toplevel() == kind {
                Some(id)
            } else {
                None
            }
        });

        id.map(|id| self.windows.remove(id))
    }
}

impl NotMappedList {
    pub fn refresh(&mut self) {
        self.windows.retain(|w| w.toplevel().alive());
    }
}
