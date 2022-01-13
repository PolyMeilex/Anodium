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

use crate::cli::{AnodiumOptions, Backend};
use crate::output_map::Output;
use crate::render::renderer::RenderFrame;
use crate::state::Anodium;

pub enum BackendEvent<'a, 'frame> {
    RequestOutputConfigure {
        output: Output,
    },
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
pub fn winit(event_loop: &mut EventLoop<'static, Anodium>, options: AnodiumOptions) -> Anodium {
    info!("Starting Anodium with winit backend");
    let display = Rc::new(RefCell::new(Display::new()));

    let (tx, rx) = channel::channel();

    let mut state = Anodium::new(
        display.clone(),
        event_loop.handle(),
        "winit".into(),
        tx,
        options,
    );

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
pub fn x11(event_loop: &mut EventLoop<'static, Anodium>, options: AnodiumOptions) -> Anodium {
    info!("Starting Anodium with x11 backend");
    let display = Rc::new(RefCell::new(Display::new()));

    let (tx, rx) = channel::channel();

    let mut state = Anodium::new(
        display.clone(),
        event_loop.handle(),
        "x11".into(),
        tx,
        options,
    );

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
pub fn udev(event_loop: &mut EventLoop<'static, Anodium>, options: AnodiumOptions) -> Anodium {
    info!("Starting Anodium on a tty using udev");
    let display = Rc::new(RefCell::new(Display::new()));

    let (session, notifier) =
        AutoSession::new(slog_scope::logger()).expect("Could not init session!");

    /*
     * Initialize the compositor
     */

    let (tx, rx) = channel::channel();

    let mut state = Anodium::new(
        display.clone(),
        event_loop.handle(),
        session.seat(),
        tx,
        options,
    );

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

pub fn auto(
    event_loop: &mut EventLoop<'static, Anodium>,
    options: AnodiumOptions,
) -> Option<Anodium> {
    match &options.backend {
        Backend::Auto => {
            if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok() {
                Some(winit(event_loop, options))
            } else {
                Some(udev(event_loop, options))
            }
        }
        Backend::X11 => Some(x11(event_loop, options)),
        Backend::Winit => Some(winit(event_loop, options)),
        Backend::Udev => Some(udev(event_loop, options)),
    }
}
