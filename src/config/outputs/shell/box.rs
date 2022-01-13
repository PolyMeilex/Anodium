use std::cell::RefCell;
use std::rc::Rc;

use imgui::Ui;
use rhai::Engine;
use rhai::{plugin::*, FLOAT, INT};

use super::widget::Widget;

thread_local! {
    static BOX_ID: RefCell<i32> = RefCell::new(0);
}

#[derive(Debug, Clone)]
pub enum Layout {
    Vertical,
    Horizontal,
}

impl Layout {
    pub fn render(&self, ui: &Ui) {
        match self {
            Layout::Horizontal => ui.same_line(),
            &Layout::Vertical => {}
        }
    }
}

//#[derive(Debug)]
pub struct BoxInner {
    id: String,
    w: u16,
    h: u16,
    x: u32,
    y: u32,
    layout: Layout,
    widgets: Vec<Rc<dyn Widget>>,
    alpha: f32,
    background: bool,
    visable: bool,
}

#[derive(Clone)]
pub struct Box {
    inner: Rc<RefCell<BoxInner>>,
}

impl Box {
    pub fn new(w: u16, h: u16, x: u32, y: u32, layout: Layout) -> Self {
        BOX_ID.with(move |id| {
            let mut id = id.borrow_mut();
            let r#box = Self {
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
                })),
            };
            *id += 1;

            r#box
        })
    }

    pub fn render(&self, ui: &Ui) {
        let inner = self.inner.borrow();
        if inner.visable {
            imgui::Window::new(&inner.id)
                .size([inner.w as _, inner.h as _], imgui::Condition::Always)
                .position([inner.x as _, inner.y as _], imgui::Condition::Always)
                .title_bar(false)
                .resizable(false)
                .bg_alpha(inner.alpha)
                .draw_background(inner.background)
                .build(ui, || {
                    for widget in &inner.widgets {
                        widget.render(ui);
                        inner.layout.render(ui);
                    }
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
