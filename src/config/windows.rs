use std::{cell::RefCell, rc::Rc};

use rhai::plugin::*;

use smithay::reexports::calloop::channel::Sender;

use super::eventloop::ConfigEvent;

use crate::window::Window as AndiumWindow;

#[derive(Debug, Clone)]
pub struct Window {
    event_sender: Sender<ConfigEvent>,
    andium_window: AndiumWindow,
}

impl Window {
    pub fn new(event_sender: Sender<ConfigEvent>, andium_window: AndiumWindow) -> Self {
        Self {
            event_sender,
            andium_window,
        }
    }
}

impl From<Window> for Dynamic {
    fn from(window: Window) -> Self {
        rhai::Dynamic::from(window)
    }
}

#[export_module]
pub mod window {
    #[rhai_fn(global)]
    pub fn maximize(window: &mut Window) {
        window
            .event_sender
            .send(ConfigEvent::Maximize(window.andium_window.clone()))
            .unwrap();
    }
    #[rhai_fn(global)]
    pub fn unmaximize(window: &mut Window) {
        window
            .event_sender
            .send(ConfigEvent::Unmaximize(window.andium_window.clone()))
            .unwrap();
    }
    #[rhai_fn(global)]
    pub fn close(window: &mut Window) {
        window
            .event_sender
            .send(ConfigEvent::Close(window.andium_window.clone()))
            .unwrap();
    }
}

#[derive(Debug, Clone)]
pub struct Windows {
    event_sender: Sender<ConfigEvent>,
    focused_window: Rc<RefCell<Option<AndiumWindow>>>,
}

impl Windows {
    pub fn new(event_sender: Sender<ConfigEvent>) -> Self {
        Self {
            event_sender,
            focused_window: Default::default(),
        }
    }

    pub fn update_focused_window(&self, window: Option<AndiumWindow>) {
        *self.focused_window.borrow_mut() = window;
    }
}

#[export_module]
pub mod windows {
    #[rhai_fn(get = "focused", pure)]
    pub fn get_focused(windows: &mut Windows) -> rhai::Dynamic {
        if let Some(andium_window) = (*windows.focused_window.borrow()).clone() {
            Window::new(windows.event_sender.clone(), andium_window).into()
        } else {
            rhai::Dynamic::UNIT
        }
    }
}

pub fn register(engine: &mut Engine) {
    let window_module = exported_module!(window);
    let windows_module = exported_module!(windows);

    engine
        .register_static_module("windows", windows_module.into())
        .register_static_module("window", window_module.into())
        .register_type::<Windows>()
        .register_type::<Window>();
}
