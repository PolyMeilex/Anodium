#[cfg(feature = "udev")]
pub mod udev;
#[cfg(feature = "winit")]
pub mod winit;

pub mod session;

use smithay::backend::renderer::gles2::Gles2Texture;
use smithay::reexports::{
    calloop::{channel::Sender, EventLoop},
    wayland_server::Display,
};
use std::{cell::RefCell, rc::Rc};

use crate::config::eventloop::ConfigEvent;
use crate::framework::backend::session::AnodiumSession;
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

#[cfg(feature = "winit")]
pub fn winit(
    event_loop: &mut EventLoop<'static, Anodium>,
    event_sender: Sender<ConfigEvent>,
) -> Anodium {
    info!("Starting Anodium with winit backend");
    let display = Rc::new(RefCell::new(Display::new()));

    let mut state = Anodium::new(
        display.clone(),
        event_loop.handle(),
        AnodiumSession::new_winit(),
        event_sender,
    );

    winit::run_winit(
        display,
        event_loop,
        &mut state,
        |event, mut ddata| {
            let state = ddata.get::<Anodium>().unwrap();
            state.handle_backend_event(event);
        },
        |event, mut ddata| {
            let state = ddata.get::<Anodium>().unwrap();
            state.process_input_event(event);
        },
    )
    .expect("Failed to initialize winit backend.");

    info!("Winit initialized");

    state
}

#[cfg(feature = "udev")]
pub fn udev(
    event_loop: &mut EventLoop<'static, Anodium>,
    event_sender: Sender<ConfigEvent>,
) -> Anodium {
    info!("Starting Anodium on a tty using udev");
    let display = Rc::new(RefCell::new(Display::new()));

    let (session, notifier) = AnodiumSession::new_udev().expect("Could not init session!");

    /*
     * Initialize the compositor
     */

    let mut state = Anodium::new(
        display.clone(),
        event_loop.handle(),
        session.clone(),
        event_sender,
    );

    udev::run_udev(
        display,
        event_loop,
        &mut state,
        session,
        notifier,
        |event, mut ddata| {
            let state = ddata.get::<Anodium>().unwrap();
            state.handle_backend_event(event);
        },
        |event, mut ddata| {
            let state = ddata.get::<Anodium>().unwrap();
            state.process_input_event(event);
        },
    )
    .expect("Failed to initialize tty backend.");

    state
}

pub fn auto(
    event_loop: &mut EventLoop<'static, Anodium>,
    event_sender: Sender<ConfigEvent>,
) -> Option<Anodium> {
    if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok() {
        #[cfg(feature = "winit")]
        {
            return Some(winit(event_loop, event_sender));
        }
    } else {
        #[cfg(feature = "udev")]
        {
            return Some(udev(event_loop, event_sender));
        }
    }
}

pub trait Backend {
    fn seat_name(&self) -> String;
    fn change_vt(&mut self, vt: i32);
}
