pub mod drain;

mod filter;
mod serializer;
pub use drain::ShellDrain;

use imgui::{StyleColor, Ui};
use rhai::plugin::*;
use rhai::{Array, Engine};

use slog::Level;

use super::widget::*;
use drain::BUFFER;

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub struct LoggerInner {
    trace: [f32; 4],
    debug: [f32; 4],
    info: [f32; 4],
    warning: [f32; 4],
    error: [f32; 4],
    critical: [f32; 4],
}

#[derive(Debug, Clone)]
pub struct Logger {
    inner: Rc<RefCell<LoggerInner>>,
}

impl Logger {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(LoggerInner {
                trace: [0.01, 0.93, 0.98, 1.0],
                debug: [0.01, 0.93, 0.98, 1.0],
                info: [0.41, 0.87, 0.67, 1.0],
                warning: [0.95, 0.48, 0.43, 1.0],
                error: [0.99, 0.27, 0.31, 1.0],
                critical: [1.0, 0.49, 0.85, 1.0],
            })),
        }
    }

    pub fn parse_color(new_color: Array) -> Option<[f32; 4]> {
        if new_color.len() == 4 {
            let mut parsed_color = [0.0, 0.0, 0.0, 0.0];
            for (i, new_color_part) in new_color.iter().enumerate() {
                if let Ok(new_color_value) = new_color_part.as_float() {
                    parsed_color[i] = new_color_value as f32;
                } else {
                    warn!("no float value in color, ignoring");
                    return None;
                }
            }

            return Some(parsed_color);
        }
        return None;
    }
}

impl Widget for Logger {
    fn render(&self, ui: &Ui, _config_tx: &Sender<ConfigEvent>) {
        let inner = self.inner.borrow();
        let buffer = BUFFER.lock().unwrap();
        for (level, line) in buffer.iter() {
            let token = match level {
                Level::Trace => ui.push_style_color(StyleColor::Text, inner.trace),
                Level::Debug => ui.push_style_color(StyleColor::Text, inner.debug),
                Level::Info => ui.push_style_color(StyleColor::Text, inner.info),
                Level::Warning => ui.push_style_color(StyleColor::Text, inner.warning),
                Level::Error => ui.push_style_color(StyleColor::Text, inner.error),
                Level::Critical => ui.push_style_color(StyleColor::Text, inner.critical),
            };

            ui.text_wrapped(line);
            token.end();
        }
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
