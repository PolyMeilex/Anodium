use smithay::utils::{Logical, Point};

use crate::utils::AsWlSurface;

use crate::desktop_layout::window::{WindowSurface, Window};

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

    pub fn remove(&mut self, kind: &WindowSurface) -> Option<Window> {
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
