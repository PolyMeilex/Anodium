#[macro_use]
extern crate log;

#[cfg(feature = "udev")]
pub mod udev;
#[cfg(feature = "winit")]
pub mod winit;
#[cfg(feature = "x11")]
pub mod x11;

pub mod utils;

use calloop::channel::Channel;
use smithay::{
    backend::input::{InputBackend, InputEvent},
    backend::renderer::gles2::{Gles2Renderer, Gles2Texture},
    backend::session::auto::AutoSession,
    reexports::{calloop::EventLoop, wayland_server::Display},
    utils::{Logical, Rectangle},
    wayland::output::Output as SmithayOutput,
    wayland::{self, output::PhysicalProperties},
};

use std::{cell::RefCell, rc::Rc};

pub trait OutputHandler {
    /// Request output mode for output that is being built
    fn ask_for_output_mode(
        &mut self,
        _name: &str,
        _physical_properties: &PhysicalProperties,
        modes: &[wayland::output::Mode],
    ) -> wayland::output::Mode {
        modes[0]
    }

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
        output: Option<&SmithayOutput>,
    );
}

pub trait BackendHandler: OutputHandler + InputHandler {
    fn wl_display(&mut self) -> Rc<RefCell<Display>>;

    fn send_frames(&mut self);

    fn start_compositor(&mut self);
    fn close_compositor(&mut self);
}

#[derive(Debug)]
pub enum BackendRequest {
    ChangeVT(i32),
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
    handler: &mut D,
    rx: Channel<BackendRequest>,
    backend: PreferedBackend,
) where
    D: BackendHandler + 'static,
{
    match backend {
        PreferedBackend::Auto => {
            if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok() {
                info!("Starting with winit backend");
                winit::run_winit(event_loop, handler, rx)
                    .expect("Failed to initialize winit backend.");
            } else {
                info!("Starting with x11 backend");
                x11::run_x11(event_loop, handler, rx).expect("Failed to initialize x11 backend.");
            }
        }
        PreferedBackend::X11 => {
            x11::run_x11(event_loop, handler, rx).expect("Failed to initialize x11 backend.")
        }
        PreferedBackend::Winit => {
            winit::run_winit(event_loop, handler, rx).expect("Failed to initialize winit backend.")
        }
        PreferedBackend::Udev => {
            // TODO: Call new_seat cb here and pass seat name to it
            let (session, notifier) = AutoSession::new(None).expect("Could not init session!");

            udev::run_udev(event_loop, handler, session, notifier, rx)
                .expect("Failed to initialize tty backend.");
        }
    }
}
