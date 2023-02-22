use anodium_backend::{BackendHandler, BackendState, InputHandler, OutputHandler, PreferedBackend};
use smithay::{
    backend::allocator::Format,
    backend::renderer::{Frame, Renderer},
    reexports::calloop::EventLoop,
    utils::Rectangle,
};

pub struct CalloopData {
    backend: BackendState,
}

impl InputHandler for CalloopData {
    fn process_input_event<I: smithay::backend::input::InputBackend>(
        &mut self,
        event: smithay::backend::input::InputEvent<I>,
        absolute_output: Option<&anodium_backend::OutputId>,
    ) {
    }
}
impl OutputHandler for CalloopData {
    fn output_created(&mut self, output: anodium_backend::NewOutputDescriptor) {}

    fn output_mode_updated(
        &mut self,
        output_id: &anodium_backend::OutputId,
        mode: smithay::output::Mode,
    ) {
    }

    fn output_removed(&mut self, output: &anodium_backend::OutputId) {}

    fn output_render(
        &mut self,
        renderer: &mut smithay::backend::renderer::gles2::Gles2Renderer,
        output: &anodium_backend::OutputId,
        age: usize,
        pointer_image: Option<&smithay::backend::renderer::gles2::Gles2Texture>,
    ) -> Result<
        Option<Vec<smithay::utils::Rectangle<i32, smithay::utils::Physical>>>,
        smithay::backend::SwapBuffersError,
    > {
        let mut frame = renderer
            .render(
                (i32::MAX, i32::MAX).into(),
                smithay::utils::Transform::Normal,
            )
            .unwrap();

        frame
            .clear(
                [0.0, 0.0, 0.1, 1.0],
                &[Rectangle::from_loc_and_size((0, 0), (i32::MAX, i32::MAX))],
            )
            .unwrap();

        Ok(None)
    }

    fn send_frames(&mut self, output_id: &anodium_backend::OutputId) {}
}

impl BackendHandler for CalloopData {
    fn backend_state(&mut self) -> &mut BackendState {
        &mut self.backend
    }

    fn create_dmabuf_global(&mut self, format: Vec<Format>) {}

    fn start_compositor(&mut self) {}

    fn close_compositor(&mut self) {}
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut event_loop = EventLoop::<CalloopData>::try_new()?;

    let mut data = CalloopData {
        backend: BackendState::default(),
    };

    anodium_backend::init(
        &mut event_loop,
        // &data.display.handle(),
        &mut data,
        PreferedBackend::Auto,
    );

    event_loop.run(None, &mut data, |data| {})?;

    Ok(())
}
