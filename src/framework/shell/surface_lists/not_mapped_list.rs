use std::sync::Mutex;

use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor;
use smithay::wayland::shell::xdg::{
    XdgPopupSurfaceRoleAttributes, XdgToplevelSurfaceRoleAttributes,
};

use super::super::SurfaceData;
use crate::popup::{Popup, PopupSurface};
use crate::utils::AsWlSurface;

use crate::window::{Window, WindowSurface};

#[derive(Default)]
pub struct NotMappedList {
    windows: Vec<Window>,
    popups: Vec<Popup>,
}

/// Toplevel Windows
impl NotMappedList {
    pub fn insert_window(&mut self, toplevel: WindowSurface, location: Point<i32, Logical>) {
        let mut window = Window::new(toplevel, location);
        window.self_update();
        self.windows.push(window);
    }

    #[allow(dead_code)]
    pub fn find_window<S: AsWlSurface>(&self, surface: &S) -> Option<&Window> {
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

        toplevel.and_then(|toplevel| self.remove_window(&toplevel))
    }

    pub fn remove_window(&mut self, kind: &WindowSurface) -> Option<Window> {
        let id = self.windows.iter().enumerate().find_map(|(id, win)| {
            if &win.toplevel() == kind {
                Some(id)
            } else {
                None
            }
        });

        id.map(|id| self.windows.remove(id))
    }
}

/// Popups
impl NotMappedList {
    pub fn insert_popup(&mut self, popup: PopupSurface) {
        let mut popup = Popup {
            popup,
            bbox: Default::default(),
            children: Vec::new(),
        };

        popup.self_update();

        self.popups.push(popup);
    }

    #[allow(dead_code)]
    pub fn find_popup<S: AsWlSurface>(&self, surface: &S) -> Option<&Popup> {
        if let Some(surface) = surface.as_surface() {
            self.popups.iter().find_map(|popup| {
                if popup
                    .popup_surface()
                    .get_surface()
                    .map(|s| s.as_ref().equals(surface.as_ref()))
                    .unwrap_or(false)
                {
                    Some(popup)
                } else {
                    None
                }
            })
        } else {
            None
        }
    }

    pub fn find_popup_mut<S: AsWlSurface>(&mut self, surface: &S) -> Option<&mut Popup> {
        if let Some(surface) = surface.as_surface() {
            self.popups.iter_mut().find_map(|popup| {
                if popup
                    .popup_surface()
                    .get_surface()
                    .map(|s| s.as_ref().equals(surface.as_ref()))
                    .unwrap_or(false)
                {
                    Some(popup)
                } else {
                    None
                }
            })
        } else {
            None
        }
    }

    pub fn try_popup_map(&mut self, surface: &WlSurface) -> Option<Popup> {
        let popup = self.find_popup_mut(surface).and_then(|win| {
            win.self_update();

            let popup = win.popup_surface();
            // send the initial configure if relevant
            #[allow(irrefutable_let_patterns)]
            if let PopupSurface::Xdg(ref popup) = popup {
                let initial_configure_sent = compositor::with_states(surface, |states| {
                    states
                        .data_map
                        .get::<Mutex<XdgPopupSurfaceRoleAttributes>>()
                        .unwrap()
                        .lock()
                        .unwrap()
                        .initial_configure_sent
                })
                .unwrap();
                if !initial_configure_sent {
                    popup.send_configure().unwrap();
                }
            }

            let has_buffer =
                SurfaceData::try_with(surface, |data| data.buffer.is_some()).unwrap_or(false);

            if has_buffer {
                match popup {
                    PopupSurface::Xdg(_) => {
                        let configured = compositor::with_states(surface, |states| {
                            states
                                .data_map
                                .get::<Mutex<XdgPopupSurfaceRoleAttributes>>()
                                .unwrap()
                                .lock()
                                .unwrap()
                                .configured
                        })
                        .unwrap();

                        if configured {
                            Some(popup)
                        } else {
                            None
                        }
                    }
                }
            } else {
                None
            }
        });

        popup.and_then(|popup| self.remove_popup(&popup))
    }

    pub fn remove_popup(&mut self, kind: &PopupSurface) -> Option<Popup> {
        let id = self.popups.iter().enumerate().find_map(|(id, win)| {
            if &win.popup_surface() == kind {
                Some(id)
            } else {
                None
            }
        });

        id.map(|id| self.popups.remove(id))
    }
}

impl NotMappedList {
    pub fn refresh(&mut self) {
        self.windows.retain(|w| w.toplevel().alive());
        self.popups.retain(|p| p.popup_surface().alive());
    }
}
