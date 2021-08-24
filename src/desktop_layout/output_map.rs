use std::{cell::RefCell, rc::Rc};

use smithay::{
    reexports::wayland_server::{
        protocol::wl_output::{self, WlOutput},
        Display, Global, UserDataMap,
    },
    utils::{Logical, Point, Rectangle, Size},
    wayland::{
        output::{self, Mode, PhysicalProperties},
        shell::wlr_layer,
    },
};

use crate::config::ConfigVM;

use super::layer_map::LayerMap;

#[derive(Debug)]

pub struct Output {
    name: String,
    output: output::Output,
    global: Option<Global<wl_output::WlOutput>>,
    current_mode: Mode,
    scale: f64,
    location: Point<i32, Logical>,

    active_workspace: String,
    userdata: UserDataMap,

    layer_map: LayerMap,
}

impl Output {
    fn new<N>(
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

#[derive(Debug)]

pub struct OutputMap {
    display: Rc<RefCell<Display>>,
    outputs: Vec<Output>,

    config: ConfigVM,
    logger: slog::Logger,
}

impl OutputMap {
    pub fn new(display: Rc<RefCell<Display>>, config: ConfigVM, logger: ::slog::Logger) -> Self {
        Self {
            display,
            outputs: Vec::new(),

            config,
            logger,
        }
    }

    fn arrange(&mut self) {
        let configs = self.config.arrange_outputs(&self.outputs).unwrap();

        for config in configs {
            if let Some(output) = self.outputs.get_mut(config.id()) {
                output.location = config.location();
                output
                    .output
                    .change_current_state(None, None, None, Some(output.location));

                output.layer_map.arange(output.geometry())
            }
        }
    }

    pub fn add<N>(
        &mut self,
        name: N,
        physical: PhysicalProperties,
        mode: Mode,
        active_workspace: String,
    ) -> &Output
    where
        N: AsRef<str>,
    {
        // Append the output to the end of the existing
        // outputs by placing it after the current overall
        // width
        let location = (self.width(), 0);

        let output = Output::new(
            name,
            location.into(),
            &mut *self.display.borrow_mut(),
            physical,
            mode,
            active_workspace,
            self.logger.clone(),
        );

        self.outputs.push(output);

        // We call arrange here albeit the output is only appended and
        // this would not affect windows, but arrange could re-organize
        // outputs from a configuration.
        self.arrange();

        self.outputs.last().unwrap()
    }

    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&Output) -> bool,
    {
        self.outputs.retain(f);
        self.arrange();
    }

    pub fn width(&self) -> i32 {
        // This is a simplification, we only arrange the outputs on the y axis side-by-side
        // so that the total width is simply the sum of all output widths.
        self.outputs.iter().fold(0, |acc, output| acc + output.size().w)
    }

    pub fn height(&self, x: i32) -> Option<i32> {
        // This is a simplification, we only arrange the outputs on the y axis side-by-side
        self.outputs
            .iter()
            .find(|output| {
                let geometry = output.geometry();
                x >= geometry.loc.x && x < (geometry.loc.x + geometry.size.w)
            })
            .map(|output| output.size().h)
    }

    pub fn is_empty(&self) -> bool {
        self.outputs.is_empty()
    }

    #[allow(dead_code)]
    pub fn with_primary(&self) -> Option<&Output> {
        self.outputs.get(0)
    }

    pub fn find<F>(&self, f: F) -> Option<&Output>
    where
        F: FnMut(&&Output) -> bool,
    {
        self.outputs.iter().find(f)
    }

    #[allow(dead_code)]
    pub fn find_by_output(&self, output: &wl_output::WlOutput) -> Option<&Output> {
        self.find(|o| o.output.owns(output))
    }

    pub fn find_by_name<N>(&self, name: N) -> Option<&Output>
    where
        N: AsRef<str>,
    {
        self.find(|o| o.name == name.as_ref())
    }

    #[allow(dead_code)]
    pub fn find_by_position(&self, position: Point<i32, Logical>) -> Option<&Output> {
        self.find(|o| o.geometry().contains(position))
    }

    #[allow(dead_code)]
    pub fn find_by_index(&self, index: usize) -> Option<&Output> {
        self.outputs.get(index)
    }

    pub fn iter(&self) -> std::slice::Iter<Output> {
        self.outputs.iter()
    }
    pub fn iter_mut(&mut self) -> std::slice::IterMut<Output> {
        self.outputs.iter_mut()
    }

    pub fn update<F>(&mut self, mode: Option<Mode>, scale: Option<f64>, mut f: F)
    where
        F: FnMut(&Output) -> bool,
    {
        let output = self.outputs.iter_mut().find(|o| f(&**o));

        if let Some(output) = output {
            if let Some(mode) = mode {
                output.output.delete_mode(output.current_mode);
                output
                    .output
                    .change_current_state(Some(mode), None, Some(output.scale.round() as i32), None);
                output.output.set_preferred(mode);
                output.current_mode = mode;
            }

            if let Some(scale) = scale {
                if output.scale.round() != scale.round() {
                    output.scale = scale;

                    output.output.change_current_state(
                        Some(output.current_mode),
                        None,
                        Some(scale.round() as i32),
                        None,
                    );
                }
            }
        }

        self.arrange();
    }

    pub fn refresh(&mut self) {
        for output in self.outputs.iter_mut() {
            output.layer_map.refresh();
        }
    }

    pub fn update_by_name<N: AsRef<str>>(&mut self, mode: Option<Mode>, scale: Option<f64>, name: N) {
        self.update(mode, scale, |o| o.name() == name.as_ref())
    }
}

impl OutputMap {
    pub(super) fn arrange_layers(&mut self) {
        for output in self.outputs.iter_mut() {
            output.layer_map.arange(output.geometry())
        }
    }

    pub(super) fn insert_layer(
        &mut self,
        output: Option<WlOutput>,
        surface: wlr_layer::LayerSurface,
        layer: wlr_layer::Layer,
    ) {
        let output = output.and_then(|output| self.outputs.iter_mut().find(|o| o.output.owns(&output)));

        if let Some(output) = output {
            output.layer_map.insert(surface, layer);
            output.layer_map.arange(output.geometry());
        } else if let Some(output) = self.outputs.get_mut(0) {
            output.layer_map.insert(surface, layer);
            output.layer_map.arange(output.geometry());
        }
    }

    pub(super) fn send_frames(&self, time: u32) {
        for output in self.outputs.iter() {
            output.layer_map.send_frames(time);
        }
    }
}
