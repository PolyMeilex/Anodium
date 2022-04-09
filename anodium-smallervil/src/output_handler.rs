use anodium_backend::OutputHandler;
use smithay::{desktop::space::DynamicRenderElements, wayland::seat::CursorImageStatus};

use crate::State;

impl OutputHandler for State {
    fn output_created(
        &mut self,
        output: smithay::wayland::output::Output,
        _possible_modes: Vec<smithay::wayland::output::Mode>,
    ) {
        let x = self
            .space
            .outputs()
            .fold(0, |x, o| x + o.current_mode().unwrap().size.w);

        self.space.map_output(&output, 1.0, (x, 0));
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
        let mut elems: Vec<DynamicRenderElements<_>> = Vec::new();

        let location = self
            .seat
            .get_pointer()
            .unwrap()
            .current_location()
            .to_i32_round();

        if let Some(surface) = &*self.dnd_icon.lock().unwrap() {
            if surface.as_ref().is_alive() {
                let e = anodium_framework::draw::draw_dnd_icon(surface.clone(), location);
                elems.push(Box::new(e));
            }
        }

        if let CursorImageStatus::Image(surface) = &*self.pointer_icon.lock().unwrap() {
            if surface.as_ref().is_alive() {
                let e = anodium_framework::draw::draw_cursor(surface.clone(), location);
                elems.push(Box::new(e));
            }
        }

        Ok(self
            .space
            .render_output(renderer, output, age, [0.1, 0.1, 0.1, 1.0], &elems)
            .unwrap())
    }
}
