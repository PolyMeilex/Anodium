#[macro_use]
extern crate log;

#[cfg(feature = "udev")]
pub mod udev;
#[cfg(feature = "winit")]
pub mod winit;
#[cfg(feature = "x11")]
pub mod x11;

pub mod utils;

use smithay::{
    backend::input::{InputBackend, InputEvent},
    backend::renderer::gles2::{Gles2Renderer, Gles2Texture},
    reexports::{calloop::EventLoop, wayland_server::Display},
    utils::{Logical, Rectangle},
    wayland,
    wayland::output::Output as SmithayOutput,
};

use std::{cell::RefCell, rc::Rc};

pub enum BackendState {
    Udev(udev::UdevState),
    None,
}

impl Default for BackendState {
    fn default() -> Self {
        Self::None
    }
}

impl BackendState {
    fn init_udev(&mut self, inner: udev::UdevState) {
        *self = Self::Udev(inner);
    }

    fn udev(&mut self) -> &mut udev::UdevState {
        if let Self::Udev(i) = self {
            i
        } else {
            unreachable!("Only one backend at the time");
        }
    }
}

impl BackendState {
    pub fn change_vt(&mut self, vt: i32) {
        match self {
            BackendState::Udev(inner) => inner.change_vt(vt),
            BackendState::None => {}
        }
    }

    pub fn update_mode(&mut self, output: SmithayOutput, mode: wayland::output::Mode) {
        match self {
            BackendState::Udev(inner) => inner.update_mode(output, mode),
            BackendState::None => {}
        }
    }
}

pub trait OutputHandler {
    /// Output was created
    fn output_created(&mut self, output: SmithayOutput, possible_modes: Vec<wayland::output::Mode>);

    /// Output got resized
    fn output_mode_updated(&mut self, output: &SmithayOutput, mode: wayland::output::Mode) {
        output.change_current_state(Some(mode), None, None, None);
    }

    /// Render the ouput
    fn output_render(
        &mut self,
        renderer: &mut Gles2Renderer,
        output: &SmithayOutput,
        age: usize,
        pointer_image: Option<&Gles2Texture>,
    ) -> Result<Option<Vec<Rectangle<i32, Logical>>>, smithay::backend::SwapBuffersError>;
}

pub trait InputHandler {
    /// Handle input events
    fn process_input_event<I: InputBackend>(
        &mut self,
        event: InputEvent<I>,
        absolute_output: Option<&SmithayOutput>,
    );
}

pub trait BackendHandler: OutputHandler + InputHandler {
    fn backend_state(&mut self) -> &mut BackendState;

    fn send_frames(&mut self);

    fn start_compositor(&mut self);
    fn close_compositor(&mut self);
}

#[derive(Debug, Clone)]
pub enum PreferedBackend {
    Auto,
    X11,
    Winit,
    Udev,
}

pub fn init<D>(
    event_loop: &mut EventLoop<'static, D>,
    display: Rc<RefCell<Display>>,
    handler: &mut D,
    backend: PreferedBackend,
) where
    D: BackendHandler + 'static,
{
    match backend {
        PreferedBackend::Auto => {
            if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok() {
                info!("Starting with winit backend");
                winit::run_winit(event_loop, display, handler)
                    .expect("Failed to initialize winit backend.");
            } else {
                info!("Starting with udev backend");
                udev::run_udev(event_loop, display, handler)
                    .expect("Failed to initialize tty backend.");
            }
        }
        PreferedBackend::X11 => {
            x11::run_x11(event_loop, display, handler).expect("Failed to initialize x11 backend.")
        }
        PreferedBackend::Winit => winit::run_winit(event_loop, display, handler)
            .expect("Failed to initialize winit backend."),
        PreferedBackend::Udev => {
            udev::run_udev(event_loop, display, handler)
                .expect("Failed to initialize tty backend.");
        }
    }
}
