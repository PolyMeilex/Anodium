use std::cell::RefCell;

use smithay::{
    reexports::{
        wayland_protocols::xdg_shell::server::xdg_toplevel,
        wayland_server::protocol::wl_surface::{self},
    },
    utils::{Logical, Point, Rectangle, Size},
    wayland::{
        compositor::{with_states, with_surface_tree_downward, SubsurfaceCachedState, TraversalAction},
        shell::xdg::{SurfaceCachedState, ToplevelSurface},
    },
};

#[cfg(feature = "xwayland")]
use crate::xwayland::X11Surface;
use crate::{
    animations::enter_exit::EnterExitAnimation,
    shell::{MoveAfterResizeData, MoveAfterResizeState, SurfaceData},
};

mod list;
pub use list::WindowList;

#[derive(Clone, Debug, PartialEq)]
pub enum Toplevel {
    Xdg(ToplevelSurface),
    #[cfg(feature = "xwayland")]
    X11(X11Surface),
}

impl Toplevel {
    pub fn alive(&self) -> bool {
        match *self {
            Toplevel::Xdg(ref t) => t.alive(),
            #[cfg(feature = "xwayland")]
            Toplevel::X11(ref t) => t.alive(),
        }
    }

    pub fn get_surface(&self) -> Option<&wl_surface::WlSurface> {
        match *self {
            Toplevel::Xdg(ref t) => t.get_surface(),
            #[cfg(feature = "xwayland")]
            Toplevel::X11(ref t) => t.get_surface(),
        }
    }

    /// Activate/Deactivate this window
    pub fn set_activated(&self, active: bool) {
        if let Toplevel::Xdg(ref t) = self {
            let changed = t.with_pending_state(|state| {
                if active {
                    state.states.set(xdg_toplevel::State::Activated)
                } else {
                    state.states.unset(xdg_toplevel::State::Activated)
                }
            });
            if let Ok(true) = changed {
                t.send_configure();
            }
        }
    }

    pub fn maximize(&self, size: Size<i32, Logical>) {
        if let Toplevel::Xdg(ref t) = self {
            let ret = t.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Maximized);
                state.size = Some(size);
            });
            if let Ok(_) = ret {
                t.send_configure();
            }
        }
    }

    pub fn unmaximize(&self, size: Option<Size<i32, Logical>>) {
        if let Toplevel::Xdg(ref t) = self {
            let ret = t.with_pending_state(|state| {
                state.states.unset(xdg_toplevel::State::Maximized);
                state.size = size;
            });
            if ret.is_ok() {
                t.send_configure();
            }
        }
    }

    #[allow(dead_code)]
    pub fn resize(&self, size: Size<i32, Logical>) {
        match self {
            Toplevel::Xdg(t) => {
                let res = t.with_pending_state(|state| {
                    state.size = Some(size);
                });
                if let Ok(_) = res {
                    t.send_configure();
                }
            }
            #[cfg(feature = "xwayland")]
            Toplevel::X11(t) => t.resize(size.w as u32, size.h as u32),
        };
    }
}

#[derive(Debug)]
pub struct Window {
    location: Point<i32, Logical>,
    /// A bounding box over this window and its children.
    ///
    /// Used for the fast path of the check in `matching`, and as the fall-back for the window
    /// geometry if that's not set explicitly.
    bbox: Rectangle<i32, Logical>,
    toplevel: Toplevel,

    surface: Option<wl_surface::WlSurface>,

    animation: EnterExitAnimation,
}

impl Window {
    pub fn location(&self) -> Point<i32, Logical> {
        self.location
    }

    pub fn set_location(&mut self, location: Point<i32, Logical>) {
        self.location = location;
        self.self_update();
    }

    pub fn bbox(&self) -> Rectangle<i32, Logical> {
        self.bbox
    }

    pub fn toplevel(&self) -> &Toplevel {
        &self.toplevel
    }

    pub fn surface(&self) -> Option<wl_surface::WlSurface> {
        self.surface.clone()
    }

    #[allow(dead_code)]
    pub fn animation(&self) -> EnterExitAnimation {
        self.animation
    }
}

impl Window {
    pub fn new(toplevel: Toplevel, location: Point<i32, Logical>) -> Self {
        let surface = toplevel.get_surface().cloned();
        let mut window = Window {
            location,
            bbox: Default::default(),
            toplevel,

            surface,

            animation: EnterExitAnimation::Enter(0.0),
        };
        window.self_update();
        window
    }

    pub fn maximize(&mut self, target_geometry: Rectangle<i32, Logical>) {
        let initial_window_location = self.location();
        let initial_size = self.geometry().size;

        if let Some(wl_surface) = self.toplevel().get_surface() {
            with_states(&wl_surface, |states| {
                let surface_data = states.data_map.get::<RefCell<SurfaceData>>();

                if let Some(data) = surface_data {
                    data.borrow_mut().move_after_resize_state =
                        MoveAfterResizeState::WaitingForAck(MoveAfterResizeData {
                            initial_window_location,
                            initial_size,

                            target_window_location: target_geometry.loc,
                            target_size: target_geometry.size,
                        });
                }
            })
            .unwrap();
        }

        self.toplevel.maximize(target_geometry.size);
    }

    pub fn unmaximize(&mut self) {
        let initial_window_location = self.location();
        let initial_size = self.geometry().size;

        let size = if let Some(surface) = self.toplevel().get_surface() {
            let fullscreen_state = with_states(&surface, |states| {
                let mut data = states
                    .data_map
                    .get::<RefCell<SurfaceData>>()
                    .unwrap()
                    .borrow_mut();
                let fullscreen_state = data.move_after_resize_state;

                if let MoveAfterResizeState::Current(mdata) = fullscreen_state {
                    data.move_after_resize_state = MoveAfterResizeState::WaitingForAck(MoveAfterResizeData {
                        initial_window_location,
                        initial_size,

                        target_window_location: mdata.initial_window_location,
                        target_size: mdata.initial_size,
                    });
                }

                fullscreen_state
            })
            .unwrap();

            if let MoveAfterResizeState::Current(data) = fullscreen_state {
                Some(data.initial_size)
            } else {
                None
            }
        } else {
            None
        };

        self.toplevel.unmaximize(size);
    }

    /// Finds the topmost surface under this point if any and returns it together with the location of this
    /// surface.
    pub fn matching(
        &self,
        point: Point<f64, Logical>,
    ) -> Option<(wl_surface::WlSurface, Point<i32, Logical>)> {
        if !self.toplevel.alive() {
            return None;
        }

        if !self.bbox.to_f64().contains(point) {
            return None;
        }
        // need to check more carefully
        let found = RefCell::new(None);
        if let Some(wl_surface) = self.toplevel.get_surface() {
            with_surface_tree_downward(
                wl_surface,
                self.location,
                |wl_surface, states, location| {
                    let mut location = *location;
                    let data = states.data_map.get::<RefCell<SurfaceData>>();

                    if states.role == Some("subsurface") {
                        let current = states.cached_state.current::<SubsurfaceCachedState>();
                        location += current.location;
                    }

                    let contains_the_point = data
                        .map(|data| {
                            data.borrow()
                                .contains_point(&*states.cached_state.current(), point - location.to_f64())
                        })
                        .unwrap_or(false);
                    if contains_the_point {
                        *found.borrow_mut() = Some((wl_surface.clone(), location));
                    }

                    TraversalAction::DoChildren(location)
                },
                |_, _, _| {},
                |_, _, _| {
                    // only continue if the point is not found
                    found.borrow().is_none()
                },
            );
        }
        found.into_inner()
    }

    pub fn self_update(&mut self) {
        if !self.toplevel.alive() {
            return;
        }

        let mut bounding_box = Rectangle::from_loc_and_size(self.location, (0, 0));
        if let Some(wl_surface) = self.toplevel.get_surface() {
            with_surface_tree_downward(
                wl_surface,
                self.location,
                |_, states, &loc| {
                    let mut loc = loc;
                    let data = states.data_map.get::<RefCell<SurfaceData>>();

                    if let Some(size) = data.and_then(|d| d.borrow().size()) {
                        if states.role == Some("subsurface") {
                            let current = states.cached_state.current::<SubsurfaceCachedState>();
                            loc += current.location;
                        }

                        // Update the bounding box.
                        bounding_box = bounding_box.merge(Rectangle::from_loc_and_size(loc, size));

                        TraversalAction::DoChildren(loc)
                    } else {
                        // If the parent surface is unmapped, then the child surfaces are hidden as
                        // well, no need to consider them here.
                        TraversalAction::SkipChildren
                    }
                },
                |_, _, _| {},
                |_, _, _| true,
            );
        }
        self.bbox = bounding_box;
    }

    /// Returns the geometry of this window.
    pub fn geometry(&self) -> Rectangle<i32, Logical> {
        if let Some(surface) = self.toplevel.get_surface() {
            // It's the set geometry with the full bounding box as the fallback.
            with_states(surface, |states| {
                states.cached_state.current::<SurfaceCachedState>().geometry
                // .and_then(|g| if g.size.w == 0 { None } else { Some(g) })
            })
            .unwrap()
            .unwrap_or(self.bbox)
        } else {
            self.bbox
        }
    }

    /// Sends the frame callback to all the subsurfaces in this
    /// window that requested it
    pub fn send_frame(&self, time: u32) {
        if let Some(wl_surface) = self.toplevel.get_surface() {
            with_surface_tree_downward(
                wl_surface,
                (),
                |_, _, &()| TraversalAction::DoChildren(()),
                |_, states, &()| {
                    // the surface may not have any user_data if it is a subsurface and has not
                    // yet been commited
                    SurfaceData::send_frame(&mut *states.cached_state.current(), time)
                },
                |_, _, &()| true,
            );
        }
    }

    pub fn update_animation(&mut self, delta: f64) {
        self.animation.update(delta, self.toplevel().alive());
    }

    pub fn render_location(&self) -> Point<i32, Logical> {
        let mut location = self.location.clone();

        location.y -= 1000;
        location.y += (self.animation.value() * 1000.0) as i32;

        location
    }
}
