use super::super::utils::AsWlSurface;
use smithay::desktop::Window;

#[derive(Debug, Default)]
pub struct ShellWindowList {
    windows: Vec<Window>,
}

impl ShellWindowList {
    pub fn push(&mut self, window: Window) {
        self.windows.push(window)
    }

    /// Finds the toplevel corresponding to the given `WlSurface`.
    pub fn find<S: AsWlSurface>(&mut self, surface: &S) -> Option<&Window> {
        if let Some(surface) = surface.as_surface() {
            self.windows.iter().find(|w| {
                w.toplevel()
                    .get_surface()
                    .map(|s| s.as_ref().equals(surface.as_ref()))
                    .unwrap_or(false)
            })
        } else {
            None
        }
    }

    /// Finds the toplevel corresponding to the given `WlSurface`.
    pub fn find_mut<S: AsWlSurface>(&mut self, surface: &S) -> Option<&mut Window> {
        if let Some(surface) = surface.as_surface() {
            self.windows.iter_mut().find_map(|w| {
                if w.toplevel()
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

    pub fn refresh(&mut self) {
        // self.windows.retain(|w| !w.animation().exited());
        self.windows.retain(|w| w.toplevel().alive());
    }
}
