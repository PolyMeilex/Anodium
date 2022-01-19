#[cfg(feature = "udev")]
pub mod udev;
#[cfg(feature = "winit")]
pub mod winit;
#[cfg(feature = "x11")]
pub mod x11;

use anodium_protocol::server::AnodiumProtocol;
use smithay::backend::input::{InputBackend, InputEvent};
use smithay::backend::renderer::gles2::{Gles2Renderer, Gles2Texture};
use smithay::backend::session::{auto::AutoSession, Session};
use smithay::reexports::{
    calloop::{channel, EventLoop},
    wayland_server::Display,
};
use smithay::wayland;

use std::{cell::RefCell, rc::Rc};

use crate::cli::{AnodiumOptions, Backend};
use crate::output_manager::{Output, OutputDescriptor};
use crate::state::Anodium;

pub trait OutputHandler {
    /// Request output mode for output that is being built
    fn ask_for_output_mode(
        &mut self,
        _descriptor: &OutputDescriptor,
        modes: &[wayland::output::Mode],
    ) -> wayland::output::Mode {
        modes[0]
    }

    /// Output was created
    fn output_created(&mut self, output: Output);

    /// Output got resized
    fn output_mode_updated(&mut self, output: &Output, mode: wayland::output::Mode) {
        output.change_current_state(Some(mode), None, None, None);
    }

    /// Render the ouput
    fn output_render(
        &mut self,
        renderer: &mut Gles2Renderer,
        output: &Output,
        age: usize,
        pointer_image: Option<&Gles2Texture>,
    );
}

pub trait InputHandler {
    /// Handle input events
    fn process_input_event<I: InputBackend>(
        &mut self,
        event: InputEvent<I>,
        output: Option<&Output>,
    );
}

pub trait BackendHandler: OutputHandler + InputHandler {
    // TODO(poly): I'm not a huge fan of mixing backend code with anodium specific stuff
    // This getter is used for output creation,
    // so maybe use SmithayOutput only in backend?
    fn anodium_protocol(&mut self) -> &mut AnodiumProtocol;

    fn send_frames(&mut self);

    fn start_compositor(&mut self);
    fn close_compositor(&mut self);
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

    winit::run_winit(display, event_loop, &mut state, rx)
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

    x11::run_x11(display, event_loop, &mut state, rx).expect("Failed to initialize winit backend.");

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

    udev::run_udev(display, event_loop, &mut state, session, notifier, rx)
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
