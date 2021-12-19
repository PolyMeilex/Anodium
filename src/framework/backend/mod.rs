#[cfg(feature = "udev")]
pub mod udev;
#[cfg(feature = "winit")]
pub mod winit;
#[cfg(feature = "x11")]
pub mod x11;

use smithay::backend::renderer::gles2::Gles2Texture;
use smithay::backend::session::{auto::AutoSession, Session};
use smithay::reexports::{
    calloop::{channel, EventLoop},
    wayland_server::Display,
};

use std::{cell::RefCell, rc::Rc};

use crate::output_map::Output;
use crate::render::renderer::RenderFrame;
use crate::state::Anodium;

pub enum BackendEvent<'a, 'frame> {
    OutputCreated {
        output: Output,
    },
    OutputModeUpdate {
        output: &'a Output,
    },
    OutputRender {
        frame: &'a mut RenderFrame<'frame>,
        output: &'a Output,
        pointer_image: Option<&'a Gles2Texture>,
    },
    SendFrames,

    StartCompositor,
    CloseCompositor,
}

#[derive(Debug)]
pub enum BackendRequest {
    ChangeVT(i32),
}

#[cfg(feature = "winit")]
pub fn winit(event_loop: &mut EventLoop<'static, Anodium>) -> Anodium {
    info!("Starting Anodium with winit backend");
    let display = Rc::new(RefCell::new(Display::new()));

    let (tx, rx) = channel::channel();

    let mut state = Anodium::new(display.clone(), event_loop.handle(), "winit".into(), tx);

    winit::run_winit(
        display,
        event_loop,
        &mut state,
        rx,
        |event, mut ddata| {
            let state = ddata.get::<Anodium>().unwrap();
            state.handle_backend_event(event);
        },
        |event, output, mut ddata| {
            let state = ddata.get::<Anodium>().unwrap();
            state.process_input_event(event, Some(output));
        },
    )
    .expect("Failed to initialize winit backend.");

    info!("Winit initialized");

    state
}

#[cfg(feature = "x11")]
pub fn x11(event_loop: &mut EventLoop<'static, Anodium>) -> Anodium {
    info!("Starting Anodium with x11 backend");
    let display = Rc::new(RefCell::new(Display::new()));

    let (tx, rx) = channel::channel();

    let mut state = Anodium::new(display.clone(), event_loop.handle(), "x11".into(), tx);

    x11::run_x11(
        display,
        event_loop,
        &mut state,
        rx,
        |event, mut ddata| {
            let state = ddata.get::<Anodium>().unwrap();
            state.handle_backend_event(event);
        },
        |event, output, mut ddata| {
            let state = ddata.get::<Anodium>().unwrap();
            state.process_input_event(event, Some(output));
        },
    )
    .expect("Failed to initialize winit backend.");

    info!("Winit initialized");

    state
}

#[cfg(feature = "udev")]
pub fn udev(event_loop: &mut EventLoop<'static, Anodium>) -> Anodium {
    info!("Starting Anodium on a tty using udev");
    let display = Rc::new(RefCell::new(Display::new()));

    let (session, notifier) =
        AutoSession::new(slog_scope::logger()).expect("Could not init session!");

    /*
     * Initialize the compositor
     */

    let (tx, rx) = channel::channel();

    let mut state = Anodium::new(display.clone(), event_loop.handle(), session.seat(), tx);

    udev::run_udev(
        display,
        event_loop,
        &mut state,
        session,
        notifier,
        rx,
        |event, mut ddata| {
            let state = ddata.get::<Anodium>().unwrap();
            state.handle_backend_event(event);
        },
        |event, mut ddata| {
            let state = ddata.get::<Anodium>().unwrap();
            state.process_input_event(event, None);
        },
    )
    .expect("Failed to initialize tty backend.");

    state
}

pub fn auto(event_loop: &mut EventLoop<'static, Anodium>) -> Option<Anodium> {
    if std::env::args().find(|arg| arg == "--x11").is_some() {
        #[cfg(feature = "x11")]
        {
            return Some(x11(event_loop));
        }
    } else if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok() {
        #[cfg(feature = "winit")]
        {
            return Some(winit(event_loop));
        }
    } else {
        #[cfg(feature = "udev")]
        {
            return Some(udev(event_loop));
        }
    }
}
