use anodium_backend::{BackendHandler, BackendState, InputHandler, OutputHandler, OutputId};

use smithay::{
    backend::{
        input::InputEvent,
        renderer::{Frame, Renderer},
    },
    reexports::{
        calloop::{EventLoop, LoopSignal},
        wayland_server::Display,
    },
    utils::Rectangle,
};

struct CalloopData {
    state: State,
    display: Display<State>,
}

struct State {
    loop_signal: LoopSignal,

    backend: BackendState,

    #[cfg(feature = "xwayland")]
    xwayland: XWayland<Self>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop = EventLoop::<CalloopData>::try_new()?;
    let display = Display::new()?;

    let state = State {
        loop_signal: event_loop.get_signal(),

        backend: BackendState::default(),
    };

    let dh = display.handle();
    let mut data = CalloopData { state, display };

    anodium_backend::init(
        &mut event_loop,
        &dh,
        &mut data,
        anodium_backend::PreferedBackend::Auto,
    );

    event_loop.run(None, &mut data, |data| {
        data.display.dispatch_clients(&mut data.state).unwrap();
    })?;

    Ok(())
}

impl BackendHandler for CalloopData {
    fn backend_state(&mut self) -> &mut BackendState {
        &mut self.state.backend
    }

    fn send_frames(&mut self) {}

    fn start_compositor(&mut self) {}

    fn close_compositor(&mut self) {
        self.state.loop_signal.stop();
    }
}

impl InputHandler for CalloopData {
    fn process_input_event<I: smithay::backend::input::InputBackend>(
        &mut self,
        _event: InputEvent<I>,
        _output_id: Option<&OutputId>,
    ) {
    }
}

impl OutputHandler for CalloopData {
    fn output_created(&mut self, _output: anodium_backend::NewOutputDescriptor) {}

    fn output_mode_updated(
        &mut self,
        _output_id: &OutputId,
        _mode: smithay::wayland::output::Mode,
    ) {
    }

    fn output_render(
        &mut self,
        _renderer: &mut smithay::backend::renderer::gles2::Gles2Renderer,
        _output: &OutputId,
        _age: usize,
        _pointer_image: Option<&smithay::backend::renderer::gles2::Gles2Texture>,
    ) -> Result<
        Option<Vec<smithay::utils::Rectangle<i32, smithay::utils::Physical>>>,
        smithay::backend::SwapBuffersError,
    > {
        Ok(None)
    }
}
