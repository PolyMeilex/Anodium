use core::slice;

use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
};

use crate::utils::AsWlSurface;

pub use super::{Toplevel, Window};

#[derive(Default, Debug)]
pub struct WindowList {
    pub windows: Vec<Window>,
}

impl WindowList {
    pub fn insert(&mut self, mut window: Window) {
        window.self_update();
        self.windows.insert(0, window);
    }

    pub fn refresh(&mut self) {
        self.windows.retain(|w| !w.animation.exited());

        for w in self.windows.iter_mut() {
            w.self_update();
        }
    }

    pub fn surface_under(&self, point: Point<f64, Logical>) -> Option<(WlSurface, Point<i32, Logical>)> {
        self.windows.iter().find_map(|w| w.matching(point))
    }

    pub fn bring_surface_to_top<S: AsWlSurface>(&mut self, surface: &S) {
        if let Some(surface) = surface.as_surface() {
            let found = self.windows.iter().enumerate().find(|(_, w)| {
                w.toplevel
                    .get_surface()
                    .map(|s| s.as_ref().equals(surface.as_ref()))
                    .unwrap_or(false)
            });

            if let Some((id, _)) = found {
                let winner = self.windows.remove(id);

                // Take activation away from all the windows
                for window in self.windows.iter() {
                    window.toplevel.set_activated(false);
                }

                // Give activation to our winner
                winner.toplevel.set_activated(true);

                self.windows.insert(0, winner);
            }
        }
    }

    // pub fn with_windows_from_bottom_to_top<Func>(&self, mut f: Func)
    // where
    //     Func: FnMut(&Window),
    // {
    //     for w in self.windows.iter().rev() {
    //         f(&w)
    //     }
    // }

    /// Finds the toplevel corresponding to the given `WlSurface`.
    pub fn find<S: AsWlSurface>(&self, surface: &S) -> Option<&Window> {
        surface.as_surface().and_then(|surface| {
            self.windows.iter().find_map(|w| {
                if w.toplevel
                    .get_surface()
                    .map(|s| s.as_ref().equals(surface.as_ref()))
                    .unwrap_or(false)
                {
                    Some(w)
                } else {
                    None
                }
            })
        })
    }

    /// Finds the toplevel corresponding to the given `WlSurface`.
    pub fn find_mut<S: AsWlSurface>(&mut self, surface: &S) -> Option<&mut Window> {
        if let Some(surface) = surface.as_surface() {
            self.windows.iter_mut().find_map(|w| {
                if w.toplevel
                    .get_surface()
                    .map(|s| s.as_ref().equals(surface.as_ref()))
                    .unwrap_or(false)
                {
                    Some(w)
                } else {
                    None
                }
            })
        } else {
            None
        }
    }

    /// Remove the toplevel corresponding to the given `WlSurface`.
    pub fn remove<S: AsWlSurface>(&mut self, surface: &S) -> Option<Window> {
        if let Some(surface) = surface.as_surface() {
            let id = self.windows.iter().enumerate().find_map(|(id, w)| {
                if w.toplevel
                    .get_surface()
                    .map(|s| s.as_ref().equals(surface.as_ref()))
                    .unwrap_or(false)
                {
                    Some(id)
                } else {
                    None
                }
            });
            id.map(|id| self.windows.remove(id))
        } else {
            None
        }
    }

    pub fn send_frames(&self, time: u32) {
        for window in self.windows.iter() {
            window.send_frame(time);
        }
    }

    pub fn update_animations(&mut self, delta: f64) {
        for window in self.windows.iter_mut() {
            window.update_animation(delta);
        }
    }

    pub fn iter(&self) -> slice::Iter<Window> {
        self.windows.iter()
    }

    pub fn iter_mut(&mut self) -> slice::IterMut<Window> {
        self.windows.iter_mut()
    }

    pub fn len(&self) -> usize {
        self.windows.len()
    }
}
