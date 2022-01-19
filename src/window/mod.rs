use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use smithay::desktop;
use smithay::{
    desktop::Kind,
    reexports::{
        wayland_protocols::xdg_shell::server::xdg_toplevel, wayland_server::protocol::wl_surface,
    },
    utils::{Logical, Point, Rectangle},
};

use crate::framework::surface_data::{MoveAfterResizeData, MoveAfterResizeState, SurfaceData};

#[derive(Debug, Clone)]
pub struct Window {
    inner: Rc<RefCell<Inner>>,
}

impl Window {
    pub fn new(toplevel: Kind, location: Point<i32, Logical>) -> Self {
        let mut window = Window {
            inner: Rc::new(RefCell::new(Inner {
                location,

                window: smithay::desktop::Window::new(toplevel),
            })),
        };
        window.self_update();
        window
    }

    pub fn desktop_window(&self) -> desktop::Window {
        self.inner.borrow().window.clone()
    }

    pub fn set_activated(&self, activated: bool) {
        self.inner.borrow().window.set_activated(activated);
    }

    pub fn configure(&self) {
        self.inner.borrow().window.configure();
    }

    pub fn toplevel(&self) -> Kind {
        self.inner.borrow().window.toplevel().clone()
    }
}

#[derive(Debug)]
pub struct Inner {
    location: Point<i32, Logical>,

    window: smithay::desktop::Window,
}

impl Inner {
    pub fn set_location(&mut self, location: Point<i32, Logical>) {
        self.location = location;
        // TODO: XWayland
        // self.toplevel.notify_move(location);
        self.self_update();
    }

    pub fn bbox_in_comp_space(&self) -> Rectangle<i32, Logical> {
        let mut bbox = self.window.bbox();
        bbox.loc += self.location;
        bbox
    }

    pub fn bbox_in_window_space(&self) -> Rectangle<i32, Logical> {
        self.window.bbox()
    }
}

impl Inner {
    pub fn maximize(&mut self, target_geometry: Rectangle<i32, Logical>) {
        let initial_window_location = self.location;
        let initial_size = self.geometry().size;

        let geometry = self.geometry();

        let mut target_window_location = target_geometry.loc;
        let target_size = target_geometry.size;

        // If decoration sticks out of output
        if geometry.loc.y < 0 {
            target_window_location.y -= geometry.loc.y;
        }
        if geometry.loc.x < 0 {
            target_window_location.x -= geometry.loc.x;
        }

        if let Some(surface) = self.window.toplevel().get_surface() {
            SurfaceData::with_mut(surface, |data| {
                data.move_after_resize_state =
                    MoveAfterResizeState::WaitingForAck(MoveAfterResizeData {
                        initial_window_location,
                        initial_size,

                        target_window_location,
                        target_size,
                    });
            });

            if let Kind::Xdg(ref t) = self.window.toplevel() {
                let res = t.with_pending_state(|state| {
                    state.states.set(xdg_toplevel::State::Maximized);
                    state.size = Some(target_geometry.size);
                });
                if res.is_ok() {
                    t.send_configure();
                }
            }
        }
    }

    pub fn unmaximize(&mut self) {
        let initial_window_location = self.location;
        let initial_size = self.geometry().size;

        let size = if let Some(surface) = self.window.toplevel().get_surface() {
            let fullscreen_state = SurfaceData::with_mut(surface, |data| {
                let fullscreen_state = data.move_after_resize_state;

                if let MoveAfterResizeState::Current(mdata) = fullscreen_state {
                    data.move_after_resize_state =
                        MoveAfterResizeState::WaitingForAck(MoveAfterResizeData {
                            initial_window_location,
                            initial_size,

                            target_window_location: mdata.initial_window_location,
                            target_size: mdata.initial_size,
                        });
                }

                fullscreen_state
            });

            if let MoveAfterResizeState::Current(data) = fullscreen_state {
                Some(data.initial_size)
            } else {
                None
            }
        } else {
            None
        };

        if let Kind::Xdg(ref t) = self.window.toplevel() {
            let ret = t.with_pending_state(|state| {
                state.states.unset(xdg_toplevel::State::Maximized);
                state.size = size;
            });
            if ret.is_ok() {
                t.send_configure();
            }
        }
    }

    /// Finds the topmost surface under this point if any and returns it together with the location of this
    /// surface.
    pub fn matching(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(wl_surface::WlSurface, Point<i32, Logical>)> {
        if !self.window.toplevel().alive() {
            return None;
        }

        let wl_surface = self.window.toplevel().get_surface()?;
        smithay::desktop::utils::under_from_surface_tree(wl_surface, point, self.location)
    }

    pub fn self_update(&mut self) {
        self.window.refresh();
    }

    /// Returns the geometry of this window.
    pub fn geometry(&self) -> Rectangle<i32, Logical> {
        self.window.geometry()
    }

    /// Sends the frame callback to all the subsurfaces in this
    /// window that requested it
    pub fn send_frame(&self, time: u32) {
        self.window.send_frame(time);
    }
}

impl Window {
    pub fn borrow(&self) -> Ref<Inner> {
        self.inner.borrow()
    }

    pub fn borrow_mut(&mut self) -> RefMut<Inner> {
        self.inner.borrow_mut()
    }

    pub fn location(&self) -> Point<i32, Logical> {
        self.inner.borrow().location
    }

    pub fn set_location(&mut self, location: Point<i32, Logical>) {
        self.inner.borrow_mut().set_location(location)
    }

    pub fn bbox_in_comp_space(&self) -> Rectangle<i32, Logical> {
        self.inner.borrow().bbox_in_comp_space()
    }

    #[allow(unused)]
    pub fn bbox_in_window_space(&self) -> Rectangle<i32, Logical> {
        self.inner.borrow().bbox_in_window_space()
    }

    pub fn surface(&self) -> Option<wl_surface::WlSurface> {
        self.inner.borrow().window.toplevel().get_surface().cloned()
    }

    pub fn maximize(&mut self, target_geometry: Rectangle<i32, Logical>) {
        self.inner.borrow_mut().maximize(target_geometry)
    }

    pub fn unmaximize(&mut self) {
        self.inner.borrow_mut().unmaximize()
    }

    /// Finds the topmost surface under this point if any and returns it together with the location of this
    /// surface.
    pub fn matching(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(wl_surface::WlSurface, Point<i32, Logical>)> {
        self.inner.borrow().matching(point)
    }

    pub fn self_update(&mut self) {
        self.inner.borrow_mut().self_update()
    }

    /// Returns the geometry of this window.
    pub fn geometry(&self) -> Rectangle<i32, Logical> {
        self.inner.borrow().geometry()
    }
}

impl PartialEq for Window {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.inner, &other.inner)
    }
}
