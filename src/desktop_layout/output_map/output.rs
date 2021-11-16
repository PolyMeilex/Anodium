use smithay::{
    reexports::wayland_server::{protocol::wl_output::WlOutput, Display, Global, UserDataMap},
    utils::{Logical, Point, Rectangle, Size},
    wayland::output::{self, Mode, PhysicalProperties},
};

use super::super::layer_map::LayerMap;

#[derive(Debug)]

pub struct Output {
    pub(super) name: String,
    pub(super) output: output::Output,
    global: Option<Global<WlOutput>>,
    pub(super) current_mode: Mode,
    pub(super) scale: f64,
    pub location: Point<i32, Logical>,

    active_workspace: String,
    userdata: UserDataMap,

    pub(super) layer_map: LayerMap,
}

impl Output {
    pub(super) fn new<N>(
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
            name: name.as_ref().to_owned(),
            global: Some(global),
            output,
            location,
            current_mode: mode,
            scale,

            active_workspace,
            userdata: Default::default(),

            layer_map: Default::default(),
        }
    }

    pub fn active_workspace(&self) -> &str {
        &self.active_workspace
    }
    pub fn set_active_workspace(&mut self, key: String) {
        self.active_workspace = key;
    }

    pub fn userdata(&self) -> &UserDataMap {
        &self.userdata
    }

    pub fn geometry(&self) -> Rectangle<i32, Logical> {
        let loc = self.location();
        let size = self.size();

        Rectangle { loc, size }
    }

    pub fn usable_geometry(&self) -> Rectangle<i32, Logical> {
        let mut ret = self.geometry();

        ret.loc.x += self.layer_map.exclusive_zone().left as i32;
        ret.size.w -= self.layer_map.exclusive_zone().left as i32;

        ret.loc.y += self.layer_map.exclusive_zone().top as i32;
        ret.size.h -= self.layer_map.exclusive_zone().top as i32;

        ret.size.w -= self.layer_map.exclusive_zone().left as i32;
        ret.size.h -= self.layer_map.exclusive_zone().bottom as i32;

        ret
    }

    pub fn size(&self) -> Size<i32, Logical> {
        self.current_mode
            .size
            .to_f64()
            .to_logical(self.scale)
            .to_i32_round()
    }

    pub fn location(&self) -> Point<i32, Logical> {
        self.location
    }

    pub fn scale(&self) -> f64 {
        self.scale
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn current_mode(&self) -> Mode {
        self.current_mode
    }

    pub fn layer_map(&self) -> &LayerMap {
        &self.layer_map
    }
}

impl Drop for Output {
    fn drop(&mut self) {
        self.global.take().unwrap().destroy();
    }
}
