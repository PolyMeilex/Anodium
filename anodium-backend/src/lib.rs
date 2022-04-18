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
    reexports::{
        calloop::EventLoop,
        wayland_server::{protocol::wl_output, Display},
    },
    utils::{Logical, Rectangle},
    wayland,
    wayland::output::{self, PhysicalProperties},
};

use std::{cell::RefCell, rc::Rc, str::FromStr};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct OutputId {
    id: u64,
}

#[derive(Debug)]
pub struct NewOutputDescriptor {
    pub id: OutputId,
    pub name: String,
    pub physical_properties: PhysicalProperties,

    pub prefered_mode: output::Mode,
    pub possible_modes: Vec<output::Mode>,

    pub transform: wl_output::Transform,
}

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

    pub fn update_mode(&mut self, output_id: &OutputId, mode: &wayland::output::Mode) {
        match self {
            BackendState::Udev(inner) => inner.update_mode(output_id, mode),
            BackendState::None => {}
        }
    }
}

pub trait OutputHandler {
    /// Output was created
    fn output_created(&mut self, output: NewOutputDescriptor);

    /// Output got resized
    fn output_mode_updated(&mut self, output_id: &OutputId, mode: wayland::output::Mode);

    /// Render the ouput
    fn output_render(
        &mut self,
        renderer: &mut Gles2Renderer,
        output: &OutputId,
        age: usize,
        pointer_image: Option<&Gles2Texture>,
    ) -> Result<Option<Vec<Rectangle<i32, Logical>>>, smithay::backend::SwapBuffersError>;
}

pub trait InputHandler {
    /// Handle input events
    fn process_input_event<I: InputBackend>(
        &mut self,
        event: InputEvent<I>,
        absolute_output: Option<&OutputId>,
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

impl Default for PreferedBackend {
    fn default() -> Self {
        Self::Auto
    }
}

#[derive(Debug)]
pub struct PreferedBackendParseError(String);

impl std::fmt::Display for PreferedBackendParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Unknown backend: {}", self.0)
    }
}
impl std::error::Error for PreferedBackendParseError {}

impl FromStr for PreferedBackend {
    type Err = PreferedBackendParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "x11" => Self::X11,
            "winit" => Self::Winit,
            "udev" => Self::Udev,
            "auto" => Self::Auto,
            other => return Err(PreferedBackendParseError(other.to_string())),
        })
    }
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
                #[cfg(feature = "winit")]
                winit::run_winit(event_loop, display, handler)
                    .expect("Failed to initialize winit backend.");
            } else {
                info!("Starting with udev backend");
                #[cfg(feature = "udev")]
                udev::run_udev(event_loop, display, handler)
                    .expect("Failed to initialize tty backend.");
            }
        }
        PreferedBackend::X11 =>
        {
            #[cfg(feature = "x11")]
            x11::run_x11(event_loop, display, handler).expect("Failed to initialize x11 backend.")
        }
        PreferedBackend::Winit =>
        {
            #[cfg(feature = "winit")]
            winit::run_winit(event_loop, display, handler)
                .expect("Failed to initialize winit backend.")
        }
        PreferedBackend::Udev => {
            #[cfg(feature = "udev")]
            udev::run_udev(event_loop, display, handler)
                .expect("Failed to initialize tty backend.");
        }
    }
}
