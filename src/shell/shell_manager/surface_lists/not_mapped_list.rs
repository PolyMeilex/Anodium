use std::sync::Mutex;

use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor;
use smithay::wayland::shell::xdg::XdgToplevelSurfaceRoleAttributes;

use crate::shell::SurfaceData;
use crate::utils::AsWlSurface;

use crate::window::{Window, WindowSurface};

#[derive(Default)]
pub struct NotMappedList {
    windows: Vec<Window>,
}

impl NotMappedList {
    pub fn insert(&mut self, toplevel: WindowSurface, location: Point<i32, Logical>) {
        self.windows.push(Window::new(toplevel.clone(), location));
        if let Some(w) = self.find_mut(&toplevel) {
            w.self_update()
        }
    }

    #[allow(dead_code)]
    pub fn find<S: AsWlSurface>(&self, surface: &S) -> Option<&Window> {
        if let Some(surface) = surface.as_surface() {
            self.windows.iter().find_map(|win| {
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

    pub fn find_mut<S: AsWlSurface>(&mut self, surface: &S) -> Option<&mut Window> {
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

    pub fn try_map(&mut self, surface: &WlSurface) -> Option<Window> {
        let toplevel = self.find_mut(surface).and_then(|win| {
            win.self_update();

            let toplevel = win.toplevel();
            // send the initial configure if relevant
            if let WindowSurface::Xdg(ref toplevel) = toplevel {
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

            let has_buffer =
                SurfaceData::try_with(surface, |data| data.buffer.is_some()).unwrap_or(false);

            if has_buffer {
                match toplevel {
                    WindowSurface::Xdg(_) => {
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

        toplevel.and_then(|toplevel| self.remove(&toplevel))
    }

    pub fn remove(&mut self, kind: &WindowSurface) -> Option<Window> {
        let id = self.windows.iter().enumerate().find_map(|(id, win)| {
            if &win.toplevel() == kind {
                Some(id)
            } else {
                None
            }
        });

        id.map(|id| self.windows.remove(id))
    }

    pub fn refresh(&mut self) {
        self.windows.retain(|w| w.toplevel().alive());
    }
}
