#![allow(dead_code)]

use smithay::{
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    utils::{Logical, Point},
};

pub use super::{Popup, PopupKind};
use crate::window::WindowList;

#[derive(Default)]
pub struct PopupList {
    popups: Vec<Popup>,
}

impl PopupList {
    pub fn insert(&mut self, popup: PopupKind) {
        let popup = Popup {
            popup,
            bbox: Default::default(),
        };
        self.popups.push(popup);
    }

    pub fn with_child_popups<Func>(
        &self,
        base: &WlSurface,
        initial_place: Point<i32, Logical>,
        mut f: Func,
    ) where
        Func: FnMut(&Popup, &mut Point<i32, Logical>),
    {
        fn find<Func>(
            popups: &[Popup],
            base: &WlSurface,
            initial_place: Point<i32, Logical>,
            f: &mut Func,
        ) where
            Func: FnMut(&Popup, &mut Point<i32, Logical>),
        {
            for p in popups
                .iter()
                .rev()
                .filter(move |w| w.popup.parent().as_ref() == Some(base))
            {
                let mut initial_place = initial_place;
                f(p, &mut initial_place);
                find(popups, p.popup.get_surface().unwrap(), initial_place, f)
            }
        }

        find(&self.popups, base, initial_place, &mut f);
    }

    pub fn refresh(&mut self) {
        self.popups.retain(|p| p.popup.alive());

        for p in self.popups.iter_mut() {
            p.self_update();
        }
    }

    pub fn surface_under(
        &self,
        windows: &WindowList,
        point: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<i32, Logical>)> {
        for w in windows.iter() {
            let parent = if let Some(parent) = w.surface() {
                parent
            } else {
                continue;
            };

            let parent_location = w.location();
            let parent_geometry = w.geometry();

            let mut res = None;

            self.with_child_popups(
                &parent,
                parent_location + parent_geometry.loc,
                |p, initial_place| {
                    if res.is_none() {
                        if let Some(out) = p.matching(*initial_place, point) {
                            res = Some(out);
                        }

                        let location = p.popup.location();
                        *initial_place += location;
                    }
                },
            );

            if let Some(res) = res {
                return Some(res);
            }
        }

        None
    }

    /// Finds the popup corresponding to the given `WlSurface`.
    pub fn find(&self, surface: &WlSurface) -> Option<Popup> {
        self.popups.iter().find_map(|p| {
            if p.popup
                .get_surface()
                .map(|s| s.as_ref().equals(surface.as_ref()))
                .unwrap_or(false)
            {
                Some(p.clone())
            } else {
                None
            }
        })
    }

    pub fn send_frames(&self, time: u32) {
        for p in self.popups.iter() {
            p.send_frame(time);
        }
    }

    pub fn clear(&mut self) {
        self.popups.clear();
    }
}
