use std::cell::RefCell;
use std::rc::Rc;

use calloop::channel::Sender;
use egui::{Color32, CtxRef};
use rhai::Engine;
use rhai::{plugin::*, FLOAT, INT};

use crate::config::eventloop::ConfigEvent;

use super::container::{Container, Layout};
use super::widget::Widget;

thread_local! {
    static PANEL_ID: RefCell<i32> = RefCell::new(0);
}

#[derive(Debug, Clone)]
pub enum PanelPosition {
    Top,
    Bottom,
    Left,
    Right,
}

//#[derive(Debug)]
pub struct PanelInner {
    id: String,
    size: f32,
    layout: Layout,
    position: PanelPosition,
    widgets: Vec<Rc<dyn Widget>>,
    alpha: f32,
    background: bool,
    visable: bool,
}

#[derive(Clone)]
pub struct Panel {
    inner: Rc<RefCell<PanelInner>>,
}

impl Panel {
    pub fn new(size: f32, layout: Layout, position: PanelPosition) -> Self {
        PANEL_ID.with(move |id| {
            let mut id = id.borrow_mut();
            let panel = Self {
                inner: Rc::new(RefCell::new(PanelInner {
                    id: format!("{}", id),
                    size,
                    layout,
                    position,
                    widgets: vec![],
                    alpha: 1.0,
                    background: true,
                    visable: true,
                })),
            };
            *id += 1;

            panel
        })
    }
}
impl Container for Panel {
    fn render(&self, ctx: &CtxRef, config_tx: &Sender<ConfigEvent>) {
        let inner = self.inner.borrow();
        if inner.visable {
            let mut frame = egui::Frame::window(&ctx.style());
            if !inner.background {
                frame.fill = Color32::TRANSPARENT;
                frame.stroke.width = 0.0;
            } else {
                frame.fill[3] = (inner.alpha * 255.0) as u8;
            }
            match inner.position {
                PanelPosition::Top => {
                    egui::TopBottomPanel::top(&inner.id)
                        .max_height(inner.size)
                        .min_height(inner.size)
                        .frame(frame)
                        .show(ctx, |ui| {
                            inner.layout.render(ui, &inner.widgets, config_tx);
                        });
                }
                PanelPosition::Bottom => {
                    egui::TopBottomPanel::bottom(&inner.id)
                        .max_height(inner.size)
                        .min_height(inner.size)
                        .frame(frame)
                        .show(ctx, |ui| {
                            inner.layout.render(ui, &inner.widgets, config_tx);
                        });
                }
                PanelPosition::Left => {
                    egui::SidePanel::left(&inner.id)
                        .max_width(inner.size)
                        .min_width(inner.size)
                        .frame(frame)
                        .show(ctx, |ui| {
                            inner.layout.render(ui, &inner.widgets, config_tx);
                        });
                }
                PanelPosition::Right => {
                    egui::SidePanel::right(&inner.id)
                        .max_width(inner.size)
                        .min_width(inner.size)
                        .frame(frame)
                        .show(ctx, |ui| {
                            inner.layout.render(ui, &inner.widgets, config_tx);
                        });
                }
            };
        }
    }
}

#[export_module]
pub mod panel {
    #[rhai_fn(get = "size", pure)]
    pub fn size(panel: &mut Panel) -> INT {
        panel.inner.borrow().size as _
    }

    #[rhai_fn(set = "size", pure)]
    pub fn set_size(panel: &mut Panel, size: INT) {
        panel.inner.borrow_mut().size = size as _;
    }

    #[rhai_fn(get = "layout", pure)]
    pub fn layout(panel: &mut Panel) -> Layout {
        panel.inner.borrow().layout.clone()
    }

    #[rhai_fn(set = "layout", pure)]
    pub fn set_layout(panel: &mut Panel, layout: Layout) {
        panel.inner.borrow_mut().layout = layout;
    }

    #[rhai_fn(get = "alpha", pure)]
    pub fn alpha(panel: &mut Panel) -> INT {
        panel.inner.borrow_mut().alpha as _
    }

    #[rhai_fn(set = "alpha", pure)]
    pub fn set_alpha(panel: &mut Panel, alpha: FLOAT) {
        panel.inner.borrow_mut().alpha = alpha as _;
    }

    #[rhai_fn(get = "background", pure)]
    pub fn background(panel: &mut Panel) -> bool {
        panel.inner.borrow_mut().background
    }

    #[rhai_fn(set = "background", pure)]
    pub fn set_background(panel: &mut Panel, background: bool) {
        panel.inner.borrow_mut().background = background;
    }

    #[rhai_fn(get = "visable", pure)]
    pub fn visable(panel: &mut Panel) -> bool {
        panel.inner.borrow().visable
    }

    #[rhai_fn(set = "visable", pure)]
    pub fn set_visable(panel: &mut Panel, visable: bool) {
        panel.inner.borrow_mut().visable = visable
    }

    #[rhai_fn(global)]
    pub fn add_widget(panel: &mut Panel, widget: Rc<dyn Widget>) {
        panel.inner.borrow_mut().widgets.push(widget);
    }
}

#[export_module]
pub mod position {
    pub fn top() -> PanelPosition {
        PanelPosition::Top
    }

    pub fn bottom() -> PanelPosition {
        PanelPosition::Bottom
    }

    pub fn left() -> PanelPosition {
        PanelPosition::Left
    }

    pub fn right() -> PanelPosition {
        PanelPosition::Right
    }
}

pub fn register(engine: &mut Engine) {
    let panel_module = exported_module!(panel);
    let position_module = exported_module!(position);
    engine
        .register_static_module("panel", panel_module.into())
        .register_static_module("position", position_module.into())
        .register_type::<Panel>()
        .register_type::<PanelPosition>();
}
