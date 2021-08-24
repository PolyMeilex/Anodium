use rhai::plugin::*;
use rhai::{Dynamic, EvalAltResult, ImmutableString, INT};
use smithay::reexports::drm;
use smithay::utils::{Logical, Point, Size};

#[derive(Debug, Clone)]
pub struct OutputConfig {
    pub(super) id: usize,
    pub(super) name: ImmutableString,
    pub(super) x: INT,
    pub(super) y: INT,
    pub(super) w: INT,
    pub(super) h: INT,
}

impl OutputConfig {
    pub fn id(&self) -> usize {
        self.id
    }

    pub fn location(&self) -> Point<i32, Logical> {
        (self.x as i32, self.y as i32).into()
    }

    #[allow(unused)]
    pub fn size(&self) -> Size<i32, Logical> {
        (self.w as i32, self.w as i32).into()
    }
}

#[export_module]
pub mod output_module {
    use super::OutputConfig;

    #[rhai_fn(get = "name", pure)]
    pub fn name(output: &mut OutputConfig) -> ImmutableString {
        output.name.clone()
    }

    #[rhai_fn(get = "x", pure)]
    pub fn x(output: &mut OutputConfig) -> INT {
        output.x
    }

    #[rhai_fn(set = "x")]
    pub fn set_x(output: &mut OutputConfig, x: INT) {
        output.x = x;
    }

    #[rhai_fn(get = "y", pure)]
    pub fn y(output: &mut OutputConfig) -> INT {
        output.y
    }

    #[rhai_fn(set = "y")]
    pub fn set_y(output: &mut OutputConfig, y: INT) {
        output.y = y;
    }

    #[rhai_fn(get = "w", pure)]
    pub fn w(output: &mut OutputConfig) -> INT {
        output.w
    }

    #[rhai_fn(set = "w")]
    pub fn set_w(output: &mut OutputConfig, w: INT) {
        output.w = w;
    }

    #[rhai_fn(get = "h", pure)]
    pub fn h(output: &mut OutputConfig) -> INT {
        output.h
    }

    #[rhai_fn(set = "h")]
    pub fn set_h(output: &mut OutputConfig, h: INT) {
        output.h = h;
    }
}

#[derive(Debug, Clone)]
pub struct Mode {
    pub(super) id: usize,
    pub(super) mode: drm::control::Mode,
}

#[export_module]
pub mod mode_module {
    use super::Mode;

    #[rhai_fn(get = "name", pure)]
    pub fn name(mode: &mut Mode) -> String {
        mode.mode.name().to_owned().into_string().unwrap()
    }

    #[rhai_fn(get = "clock", pure)]
    pub fn clock(mode: &mut Mode) -> INT {
        mode.mode.clock() as _
    }

    #[rhai_fn(get = "w", pure)]
    pub fn w(mode: &mut Mode) -> INT {
        mode.mode.size().0 as _
    }

    #[rhai_fn(get = "h", pure)]
    pub fn h(mode: &mut Mode) -> INT {
        mode.mode.size().1 as _
    }

    #[rhai_fn(get = "refresh", pure)]
    pub fn refresh(mode: &mut Mode) -> INT {
        mode.mode.vrefresh() as _
    }
}

pub fn register(engine: &mut Engine) {
    let module = exported_module!(output_module);

    engine
        .register_type::<OutputConfig>()
        .register_global_module(module.into());

    let module = exported_module!(mode_module);
    engine
        .register_type::<Mode>()
        .register_global_module(module.into());
}
