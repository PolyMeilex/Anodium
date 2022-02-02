pub mod drain;

mod filter;
mod serializer;
pub use drain::ShellDrain;

use egui::{Color32, Ui};
use rhai::plugin::*;
use rhai::{Array, Engine};

use slog::Level;

use super::widget::*;
use drain::BUFFER;

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct LoggerInner {
    trace: Color32,
    debug: Color32,
    info: Color32,
    warning: Color32,
    error: Color32,
    critical: Color32,
}

#[derive(Debug, Clone)]
pub struct Logger {
    inner: Rc<RefCell<LoggerInner>>,
}

impl Logger {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(LoggerInner {
                trace: Color32::from_rgb(3, 237, 250),
                debug: Color32::from_rgb(3, 237, 250),
                info: Color32::from_rgb(105, 222, 171),
                warning: Color32::from_rgb(242, 122, 110),
                error: Color32::from_rgb(252, 69, 79),
                critical: Color32::from_rgb(255, 125, 217),
            })),
        }
    }

    pub fn parse_color(new_color: Array) -> Option<Color32> {
        if new_color.len() == 3 {
            let mut parsed_color = [0, 0, 0, 255];
            for (i, new_color_part) in new_color.iter().enumerate() {
                if let Ok(new_color_value) = new_color_part.as_int() {
                    parsed_color[i] = new_color_value as u8;
                } else {
                    warn!("no float value in color, ignoring");
                    return None;
                }
            }

            Some(Color32::from_rgba_premultiplied(
                parsed_color[0],
                parsed_color[1],
                parsed_color[2],
                parsed_color[3],
            ))
        } else {
            None
        }
    }
}

impl Widget for Logger {
    fn render(&self, ui: &mut Ui, _config_tx: &Sender<ConfigEvent>) {
        let inner = self.inner.borrow();
        let mut buffer = BUFFER.lock().unwrap();
        if buffer.updated {
            ui.ctx().request_repaint();
            buffer.updated = false;
        }

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (level, line) in buffer.buffer.iter() {
                    let mut label = egui::RichText::new(line);
                    label = match level {
                        Level::Trace => label.color(inner.trace),
                        Level::Debug => label.color(inner.debug),
                        Level::Info => label.color(inner.info),
                        Level::Warning => label.color(inner.warning),
                        Level::Error => label.color(inner.error),
                        Level::Critical => label.color(inner.critical),
                    };
                    ui.add(egui::Label::new(label).wrap(true));
                }
            })
    }
}

#[export_module]
pub mod logger {
    #[rhai_fn(global)]
    pub fn convert(logger: &mut Logger) -> Rc<dyn Widget> {
        Rc::new(logger.clone())
    }

    #[rhai_fn(set = "c_trace", pure)]
    pub fn set_trace(logger: &mut Logger, trace: Array) {
        if let Some(new_color) = Logger::parse_color(trace) {
            logger.inner.borrow_mut().trace = new_color;
        }
    }

    #[rhai_fn(set = "c_debug", pure)]
    pub fn set_debug(logger: &mut Logger, debug: Array) {
        if let Some(new_color) = Logger::parse_color(debug) {
            logger.inner.borrow_mut().debug = new_color;
        }
    }

    #[rhai_fn(set = "c_info", pure)]
    pub fn set_info(logger: &mut Logger, info: Array) {
        if let Some(new_color) = Logger::parse_color(info) {
            logger.inner.borrow_mut().info = new_color;
        }
    }

    #[rhai_fn(set = "c_warning", pure)]
    pub fn set_warning(logger: &mut Logger, warning: Array) {
        if let Some(new_color) = Logger::parse_color(warning) {
            logger.inner.borrow_mut().warning = new_color;
        }
    }

    #[rhai_fn(set = "c_error", pure)]
    pub fn set_error(logger: &mut Logger, error: Array) {
        if let Some(new_color) = Logger::parse_color(error) {
            logger.inner.borrow_mut().error = new_color;
        }
    }

    #[rhai_fn(set = "c_critical", pure)]
    pub fn set_critical(logger: &mut Logger, critical: Array) {
        if let Some(new_color) = Logger::parse_color(critical) {
            logger.inner.borrow_mut().critical = new_color;
        }
    }
}

pub fn register(engine: &mut Engine) {
    let logger_module = exported_module!(logger);
    engine
        .register_static_module("logger", logger_module.into())
        .register_type::<Logger>();
}
