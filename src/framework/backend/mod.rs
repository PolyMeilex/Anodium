#[cfg(feature = "udev")]
pub mod udev;
#[cfg(feature = "winit")]
pub mod winit;
#[cfg(feature = "x11")]
pub mod x11;

use anodium_protocol::server::AnodiumProtocol;
use calloop::channel::Channel;
use smithay::{
    backend::input::{InputBackend, InputEvent},
    backend::renderer::gles2::{Gles2Renderer, Gles2Texture},
    backend::session::auto::AutoSession,
    reexports::{calloop::EventLoop, wayland_server::Display},
    utils::{Logical, Rectangle},
    wayland,
};

use std::{cell::RefCell, rc::Rc};

use crate::{
    cli::{self, Backend},
    output_manager::{Output, OutputDescriptor},
};

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
    ) -> Result<Option<Vec<Rectangle<i32, Logical>>>, smithay::backend::SwapBuffersError>;
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
    fn wl_display(&mut self) -> Rc<RefCell<Display>>;

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

pub fn auto<D>(
    event_loop: &mut EventLoop<'static, D>,
    handler: &mut D,
    rx: Channel<BackendRequest>,
    backend: cli::Backend,
) where
    D: BackendHandler + 'static,
{
    match backend {
        Backend::Auto => {
            if std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok() {
                info!("Starting with winit backend");
                winit::run_winit(event_loop, handler, rx)
                    .expect("Failed to initialize winit backend.");
            } else {
                info!("Starting with x11 backend");
                x11::run_x11(event_loop, handler, rx).expect("Failed to initialize x11 backend.");
            }
        }
        Backend::X11 => {
            x11::run_x11(event_loop, handler, rx).expect("Failed to initialize x11 backend.")
        }
        Backend::Winit => {
            winit::run_winit(event_loop, handler, rx).expect("Failed to initialize winit backend.")
        }
        Backend::Udev => {
            // TODO: Call new_seat cb here and pass seat name to it
            let (session, notifier) =
                AutoSession::new(slog_scope::logger()).expect("Could not init session!");

            udev::run_udev(event_loop, handler, session, notifier, rx)
                .expect("Failed to initialize tty backend.");
        }
    }
}
