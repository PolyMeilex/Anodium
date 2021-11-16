use std::{cell::RefCell, rc::Rc};

use smithay::{
    reexports::wayland_server::{
        protocol::wl_output::{self, WlOutput},
        Display,
    },
    utils::{Logical, Point},
    wayland::output::{Mode, PhysicalProperties},
};

use crate::config::ConfigVM;

use super::layer_map::LayerSurface;

mod output;
pub use output::Output;

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
                output.set_location(config.location());
                output.change_current_state(None, None, None, Some(output.location()));

                let geometry = output.geometry();
                output.layer_map_mut().arange(geometry)
            }
        }
    }

    pub fn add(&mut self, mut output: Output) -> &Output {
        // Append the output to the end of the existing
        // outputs by placing it after the current overall
        // width
        let location = (self.width(), 0);

        output.set_location(location.into());

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
        self.outputs
            .iter()
            .fold(0, |acc, output| acc + output.size().w)
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
        self.find(|o| o.inner_output().owns(output))
    }

    pub fn find_by_name<N>(&self, name: N) -> Option<&Output>
    where
        N: AsRef<str>,
    {
        self.find(|o| &o.name() == name.as_ref())
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
                let scale = output.scale().round() as i32;
                let current_mode = output.current_mode();

                {
                    let output = output.inner_output();
                    output.delete_mode(current_mode);
                    output.change_current_state(Some(mode), None, Some(scale), None);
                    output.set_preferred(mode);
                }
                output.set_current_mode(mode);
            }

            if let Some(scale) = scale {
                if output.scale().round() as u32 != scale.round() as u32 {
                    let current_mode = output.current_mode();

                    output.set_scale(scale);

                    output.inner_output().change_current_state(
                        Some(current_mode),
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
            output.layer_map_mut().refresh();
        }
    }

    pub fn update_by_name<N: AsRef<str>>(
        &mut self,
        mode: Option<Mode>,
        scale: Option<f64>,
        name: N,
    ) {
        self.update(mode, scale, |o| o.name() == name.as_ref())
    }
}

impl OutputMap {
    pub(super) fn arrange_layers(&mut self) {
        for output in self.outputs.iter_mut() {
            let geometry = output.geometry();
            output.layer_map_mut().arange(geometry);
        }
    }

    pub(super) fn insert_layer(&mut self, output: Option<WlOutput>, layer: LayerSurface) {
        let output = output.and_then(|output| {
            self.outputs
                .iter_mut()
                .find(|o| o.inner_output().owns(&output))
        });

        if let Some(output) = output {
            let geometry = output.geometry();
            let mut layer_map = output.layer_map_mut();

            layer_map.insert(layer);
            layer_map.arange(geometry);
        } else if let Some(output) = self.outputs.get_mut(0) {
            let geometry = output.geometry();
            let mut layer_map = output.layer_map_mut();

            layer_map.insert(layer);
            layer_map.arange(geometry);
        }
    }

    pub(super) fn send_frames(&self, time: u32) {
        for output in self.outputs.iter() {
            output.layer_map().send_frames(time);
        }
    }
}
