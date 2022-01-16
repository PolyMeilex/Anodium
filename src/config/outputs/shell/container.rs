use std::rc::Rc;

pub use calloop::channel::Sender;
use egui::{CtxRef, Ui};
use rhai::{plugin::*, INT};

pub use crate::config::eventloop::ConfigEvent;

use super::panel::{Panel as PanelContainer, PanelPosition};
use super::r#box::Box as BoxContainer;
use super::widget::Widget;

#[derive(Debug, Clone)]
pub enum Layout {
    Vertical,
    Horizontal,
}

impl Layout {
    pub fn render(
        &self,
        ui: &mut Ui,
        widgets: &Vec<Rc<dyn Widget>>,
        config_tx: &Sender<ConfigEvent>,
    ) {
        match self {
            Layout::Horizontal => ui.horizontal(|ui| {
                for widget in widgets {
                    widget.render(ui, config_tx);
                }
            }),
            Layout::Vertical => ui.vertical(|ui| {
                for widget in widgets {
                    widget.render(ui, config_tx);
                }
            }),
        };
    }
}

pub trait Container {
    fn render(&self, ctx: &CtxRef, config_tx: &Sender<ConfigEvent>);
}

impl std::fmt::Debug for Box<dyn Container> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "derp")
    }
}

impl Clone for Box<dyn Container> {
    fn clone(&self) -> Box<dyn Container> {
        self.to_owned()
    }
}

#[export_module]
pub mod container {
    #[rhai_fn(name = "box")]
    pub fn container_box(w: INT, h: INT, x: INT, y: INT, layout: Layout) -> BoxContainer {
        BoxContainer::new(w as _, h as _, x as _, y as _, layout)
    }

    #[rhai_fn(name = "panel")]
    pub fn container_panel(w: INT, layout: Layout, position: PanelPosition) -> PanelContainer {
        PanelContainer::new(w as _, layout, position)
    }
}

#[export_module]
pub mod layout {
    pub fn vertical() -> Layout {
        Layout::Vertical
    }
    pub fn horizontal() -> Layout {
        Layout::Horizontal
    }
}

pub fn register(engine: &mut Engine) {
    let container_module = exported_module!(container);
    let layout_module = exported_module!(layout);
    engine
        .register_static_module("container", container_module.into())
        .register_static_module("layout", layout_module.into())
        .register_type::<Layout>();
}
