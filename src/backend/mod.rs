#[cfg(feature = "udev")]
pub mod udev;
#[cfg(feature = "winit")]
pub mod winit;

pub mod session;

use smithay::backend::renderer::gles2::Gles2Texture;
use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
use smithay::wayland::output::Mode;
use std::sync::atomic::Ordering;
use std::{cell::RefCell, rc::Rc};

use crate::backend::session::AnodiumSession;
use crate::desktop_layout::Output;
use crate::render::renderer::RenderFrame;
use crate::state::Anodium;

pub enum BackendEvent<'a, 'frame> {
    OutputCreated {
        output: Output,
    },
    OutputModeUpdate {
        output: &'a Output,
        mode: Mode,
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
pub fn winit(event_loop: &mut EventLoop<'static, Anodium>) -> Anodium {
    info!("Starting Anodium with winit backend");
    let display = Rc::new(RefCell::new(Display::new()));

    let mut state = Anodium::init(
        display.clone(),
        event_loop.handle(),
        AnodiumSession::new_winit(),
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
pub fn udev(event_loop: &mut EventLoop<'static, Anodium>) -> Anodium {
    info!("Starting Anodium on a tty using udev");
    let display = Rc::new(RefCell::new(Display::new()));

    let (session, notifier) = AnodiumSession::new_udev().expect("Could not init session!");

    /*
     * Initialize the compositor
     */

    let mut state = Anodium::init(display.clone(), event_loop.handle(), session.clone());

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

pub fn auto() {
    let mut event_loop = EventLoop::try_new().unwrap();

    if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok() {
        #[cfg(feature = "winit")]
        {
            let state = winit(&mut event_loop);
            run_loop(state, event_loop)
        }
    } else {
        #[cfg(feature = "udev")]
        {
            let state = udev(&mut event_loop);
            run_loop(state, event_loop)
        }
    }
}

fn run_loop(mut state: Anodium, mut event_loop: EventLoop<'static, Anodium>) {
    let signal = event_loop.get_signal();
    event_loop
        .run(None, &mut state, |state| {
            if state.desktop_layout.output_map.is_empty() || !state.running.load(Ordering::SeqCst) {
                signal.stop();
            }

            state.display.borrow_mut().flush_clients(&mut ());
            state.update();
        })
        .unwrap();
}

pub trait Backend {
    fn seat_name(&self) -> String;
    fn change_vt(&mut self, vt: i32);
}
