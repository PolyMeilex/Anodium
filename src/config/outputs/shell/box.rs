use std::cell::{Cell, RefCell};
use std::rc::Rc;

use calloop::channel::Sender;
use egui::{Color32, CtxRef};
use rhai::Engine;
use rhai::{plugin::*, FLOAT, INT};

use crate::config::eventloop::ConfigEvent;

use super::widget::Widget;

thread_local! {
    static BOX_ID: Cell<i32> = Cell::new(0);
}

#[derive(Debug, Clone)]
pub enum Layout {
    Vertical,
    Horizontal,
}

//#[derive(Debug)]
pub struct BoxInner {
    id: String,
    w: f32,
    h: f32,
    x: f32,
    y: f32,
    layout: Layout,
    widgets: Vec<Rc<dyn Widget>>,
    alpha: f32,
    background: bool,
    visable: bool,
    scroll: bool,
}

#[derive(Clone)]
pub struct Box {
    inner: Rc<RefCell<BoxInner>>,
}

impl Box {
    pub fn new(w: u16, h: u16, x: u32, y: u32, layout: Layout) -> Self {
        BOX_ID.with(move |id_cell| {
            let id = id_cell.get();
            id_cell.set(id + 1);

            Self {
                inner: Rc::new(RefCell::new(BoxInner {
                    id: format!("{}", id),
                    w,
                    h,
                    x,
                    y,
                    layout,
                    widgets: vec![],
                    alpha: 1.0,
                    background: true,
                    visable: true,
                    scroll: true,
                })),
            }
        })
    }

    pub fn render(&self, ctx: &CtxRef, config_tx: &Sender<ConfigEvent>) {
        let inner = self.inner.borrow();
        if inner.visable {
            let mut frame = egui::Frame::window(&ctx.style());
            if !inner.background {
                frame.fill = Color32::TRANSPARENT;
                frame.stroke.width = 0.0;
            } else {
                frame.fill[3] = (inner.alpha * 255.0) as u8;
            }
            egui::containers::Window::new(&inner.id)
                .resize(|r| r.with_stroke(true)) //BUG : https://github.com/emilk/egui/issues/498, this work arounds it
                .frame(frame)
                .fixed_pos([inner.x, inner.y])
                .fixed_size([inner.w, inner.h])
                .title_bar(false)
                .collapsible(false)
                .scroll2([inner.scroll, inner.scroll])
                .show(ctx, |ui| match inner.layout {
                    Layout::Horizontal => ui.horizontal(|ui| {
                        for widget in &inner.widgets {
                            widget.render(ui, config_tx);
                        }
                    }),
                    Layout::Vertical => ui.vertical(|ui| {
                        for widget in &inner.widgets {
                            widget.render(ui, config_tx);
                        }
                    }),
                });
        }
    }
}

#[export_module]
pub mod r#box {
    pub fn new_raw(w: INT, h: INT, x: INT, y: INT, layout: Layout) -> Box {
        Box::new(w as _, h as _, x as _, y as _, layout)
    }

    #[rhai_fn(get = "w", pure)]
    pub fn w(r#box: &mut Box) -> INT {
        r#box.inner.borrow().w as _
    }

    #[rhai_fn(set = "w", pure)]
    pub fn set_w(r#box: &mut Box, w: INT) {
        r#box.inner.borrow_mut().w = w as _;
    }

    #[rhai_fn(get = "h", pure)]
    pub fn h(r#box: &mut Box) -> INT {
        r#box.inner.borrow().h as _
    }

    #[rhai_fn(set = "h", pure)]
    pub fn set_h(r#box: &mut Box, h: INT) {
        r#box.inner.borrow_mut().h = h as _;
    }

    #[rhai_fn(get = "x", pure)]
    pub fn x(r#box: &mut Box) -> INT {
        r#box.inner.borrow().x as _
    }

    #[rhai_fn(set = "x", pure)]
    pub fn set_x(r#box: &mut Box, x: INT) {
        r#box.inner.borrow_mut().x = x as _;
    }

    #[rhai_fn(get = "y", pure)]
    pub fn y(r#box: &mut Box) -> INT {
        r#box.inner.borrow().y as _
    }

    #[rhai_fn(set = "y", pure)]
    pub fn set_y(r#box: &mut Box, y: INT) {
        r#box.inner.borrow_mut().y = y as _;
    }

    #[rhai_fn(get = "layout", pure)]
    pub fn layout(r#box: &mut Box) -> Layout {
        r#box.inner.borrow().layout.clone()
    }

    #[rhai_fn(set = "layout", pure)]
    pub fn set_layout(r#box: &mut Box, layout: Layout) {
        r#box.inner.borrow_mut().layout = layout;
    }

    #[rhai_fn(get = "alpha", pure)]
    pub fn alpha(r#box: &mut Box) -> INT {
        r#box.inner.borrow_mut().alpha as _
    }

    #[rhai_fn(set = "alpha", pure)]
    pub fn set_alpha(r#box: &mut Box, alpha: FLOAT) {
        r#box.inner.borrow_mut().alpha = alpha as _;
    }

    #[rhai_fn(get = "background", pure)]
    pub fn background(r#box: &mut Box) -> bool {
        r#box.inner.borrow_mut().background
    }

    #[rhai_fn(set = "background", pure)]
    pub fn set_background(r#box: &mut Box, background: bool) {
        r#box.inner.borrow_mut().background = background;
    }

    #[rhai_fn(get = "visable", pure)]
    pub fn visable(r#box: &mut Box) -> bool {
        r#box.inner.borrow().visable
    }

    #[rhai_fn(set = "visable", pure)]
    pub fn set_visable(r#box: &mut Box, visable: bool) {
        r#box.inner.borrow_mut().visable = visable
    }

    #[rhai_fn(set = "scroll", pure)]
    pub fn set_scroll(r#box: &mut Box, scroll: bool) {
        r#box.inner.borrow_mut().scroll = scroll;
    }

    #[rhai_fn(get = "scroll", pure)]
    pub fn scroll(r#box: &mut Box) -> bool {
        r#box.inner.borrow().scroll
    }

    #[rhai_fn(global)]
    pub fn add_widget(r#box: &mut Box, widget: Rc<dyn Widget>) {
        //info!("adding widget: {:?}", widget);
        r#box.inner.borrow_mut().widgets.push(widget);
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
    let box_module = exported_module!(r#box);
    let layout_module = exported_module!(layout);
    engine
        .register_static_module("box", box_module.into())
        .register_static_module("layout", layout_module.into())
        .register_type::<Box>()
        .register_type::<Layout>();
}
