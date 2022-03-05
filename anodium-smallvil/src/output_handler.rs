use anodium_backend::OutputHandler;

use crate::State;

impl OutputHandler for State {
    fn output_created(
        &mut self,
        output: smithay::wayland::output::Output,
        _possible_modes: Vec<smithay::wayland::output::Mode>,
    ) {
        self.space.map_output(&output, 1.0, (0, 0));
    }

    fn output_render(
        &mut self,
        renderer: &mut smithay::backend::renderer::gles2::Gles2Renderer,
        output: &smithay::wayland::output::Output,
        age: usize,
        _pointer_image: Option<&smithay::backend::renderer::gles2::Gles2Texture>,
    ) -> Result<
        Option<Vec<smithay::utils::Rectangle<i32, smithay::utils::Logical>>>,
        smithay::backend::SwapBuffersError,
    > {
        Ok(self
            .space
            .render_output(renderer, output, age, [0.1, 0.1, 0.1, 1.0], &[])
            .unwrap())
    }
}
