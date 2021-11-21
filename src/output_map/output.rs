use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use smithay::reexports::wayland_server::protocol::wl_output::Transform;
use smithay::{
    reexports::wayland_server::{protocol::wl_output::WlOutput, Display, Global, UserDataMap},
    utils::{Logical, Point, Rectangle, Size},
    wayland::output::{self, Mode, PhysicalProperties},
};

use super::layer_map::LayerMap;

#[derive(Debug)]
struct Inner {
    pub(super) name: String,
    pub(super) output: output::Output,
    global: Option<Global<WlOutput>>,
    pub(super) current_mode: Mode,
    pub(super) scale: f64,
    location: Point<i32, Logical>,

    active_workspace: String,
    userdata: UserDataMap,

    pub(super) layer_map: LayerMap,
}

#[derive(Debug, Clone)]
pub struct Output {
    inner: Rc<RefCell<Inner>>,
}

impl Output {
    pub fn new<N>(
        name: N,
        location: Point<i32, Logical>,
        display: &mut Display,
        physical: PhysicalProperties,
        mode: Mode,
        active_workspace: String,
        logger: slog::Logger,
    ) -> Self
    where
        N: AsRef<str>,
    {
        let (output, global) = output::Output::new(display, name.as_ref().into(), physical, logger);

        let scale = 1.0f64;

        output.change_current_state(Some(mode), None, Some(scale.round() as i32), Some(location));
        output.set_preferred(mode);

        Self {
            inner: Rc::new(RefCell::new(Inner {
                name: name.as_ref().to_owned(),
                global: Some(global),
                output,
                location,
                current_mode: mode,
                scale,

                active_workspace,
                userdata: Default::default(),

                layer_map: Default::default(),
            })),
        }
    }

    pub fn active_workspace(&self) -> String {
        self.inner.borrow().active_workspace.clone()
    }
    pub fn set_active_workspace(&mut self, key: String) {
        self.inner.borrow_mut().active_workspace = key;
    }

    pub fn location(&self) -> Point<i32, Logical> {
        self.inner.borrow().location
    }
    pub fn set_location(&mut self, location: Point<i32, Logical>) {
        self.inner.borrow_mut().location = location;
    }

    pub fn userdata(&self) -> Ref<UserDataMap> {
        Ref::map(self.inner.borrow(), |b| &b.userdata)
    }

    pub fn geometry(&self) -> Rectangle<i32, Logical> {
        let loc = self.location();
        let size = self.size();

        Rectangle { loc, size }
    }

    pub fn usable_geometry(&self) -> Rectangle<i32, Logical> {
        let mut ret = self.geometry();

        let inner = self.inner.borrow();

        ret.loc.x += inner.layer_map.exclusive_zone().left as i32;
        ret.size.w -= inner.layer_map.exclusive_zone().left as i32;

        ret.loc.y += inner.layer_map.exclusive_zone().top as i32;
        ret.size.h -= inner.layer_map.exclusive_zone().top as i32;

        ret.size.w -= inner.layer_map.exclusive_zone().left as i32;
        ret.size.h -= inner.layer_map.exclusive_zone().bottom as i32;

        ret
    }

    pub fn size(&self) -> Size<i32, Logical> {
        let inner = self.inner.borrow();

        inner
            .current_mode
            .size
            .to_f64()
            .to_logical(inner.scale)
            .to_i32_round()
    }

    pub fn scale(&self) -> f64 {
        self.inner.borrow().scale
    }

    pub fn set_scale(&self, scale: f64) {
        self.inner.borrow_mut().scale = scale;
    }

    pub fn name(&self) -> String {
        self.inner.borrow().name.clone()
    }

    pub fn current_mode(&self) -> Mode {
        self.inner.borrow().current_mode
    }

    pub fn layer_map(&self) -> Ref<LayerMap> {
        Ref::map(self.inner.borrow(), |b| &b.layer_map)
    }

    pub fn layer_map_mut(&mut self) -> RefMut<LayerMap> {
        RefMut::map(self.inner.borrow_mut(), |b| &mut b.layer_map)
    }

    pub fn inner_output(&self) -> Ref<output::Output> {
        Ref::map(self.inner.borrow(), |b| &b.output)
    }

    pub fn change_current_state(
        &self,
        new_mode: Option<Mode>,
        new_transform: Option<Transform>,
        new_scale: Option<i32>,
        new_location: Option<Point<i32, Logical>>,
    ) {
        self.inner.borrow().output.change_current_state(
            new_mode,
            new_transform,
            new_scale,
            new_location,
        );
    }

    pub fn set_current_mode(&mut self, mode: Mode) {
        self.inner.borrow_mut().current_mode = mode;
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.global.take().unwrap().destroy();
    }
}
