use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

use egui::Ui;
use rhai::plugin::*;
use rhai::Engine;

use crate::output_manager::Output;

use super::widget::*;

#[derive(Debug, Clone)]
struct Inner {
    old_fps: u32,
    old_fps_timeout: Instant,
}

#[derive(Debug, Clone)]
pub struct Fps {
    output: Output,
    inner: Rc<RefCell<Inner>>,
}

impl Fps {
    pub fn new(output: Output) -> Self {
        let current_fps = output.get_fps();
        Self {
            output,
            inner: Rc::new(RefCell::new(Inner {
                old_fps: current_fps,
                old_fps_timeout: Instant::now(),
            })),
        }
    }
}

impl Widget for Fps {
    #[cfg(feature = "debug")]
    fn render(&self, ui: &mut Ui, _config_tx: &Sender<ConfigEvent>) {
        let mut inner = self.inner.borrow_mut();
        if inner.old_fps_timeout.elapsed().as_secs() >= 1 {
            inner.old_fps = self.output.get_fps();
            ui.ctx().request_repaint();
            inner.old_fps_timeout = Instant::now();
        }
        ui.label(format!("FPS: {}", inner.old_fps as u32));
        //ui.label(format!("FPS: {}", 0));
    }

    #[cfg(not(feature = "debug"))]
    fn render(&self, ui: &mut Ui, _config_tx: &Sender<ConfigEvent>) {
        ui.label("FPS: debug feature not enabled");
    }
}

#[export_module]
pub mod fps {
    #[rhai_fn(global)]
    pub fn convert(fps: &mut Fps) -> Rc<dyn Widget> {
        Rc::new(fps.clone())
    }
}

pub fn register(engine: &mut Engine) {
    let fps_module = exported_module!(fps);
    engine
        .register_static_module("fps", fps_module.into())
        .register_type::<Fps>();
}
