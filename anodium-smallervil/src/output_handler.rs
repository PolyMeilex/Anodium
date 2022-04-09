use crate::State;

use anodium_backend::{utils::cursor::PointerElement, OutputHandler};

use smithay::{
    backend::renderer::gles2::{Gles2Renderer, Gles2Texture},
    desktop::space::SurfaceTree,
};

smithay::custom_elements! {
    pub CustomElem<=Gles2Renderer>;
    SurfaceTree=SurfaceTree,
    PointerElement=PointerElement,
}

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
        renderer: &mut Gles2Renderer,
        output: &smithay::wayland::output::Output,
        age: usize,
        pointer_image: Option<&Gles2Texture>,
    ) -> Result<
        Option<Vec<smithay::utils::Rectangle<i32, smithay::utils::Logical>>>,
        smithay::backend::SwapBuffersError,
    > {
        let mut elems: Vec<CustomElem> = Vec::new();

        let location = self
            .seat
            .get_pointer()
            .unwrap()
            .current_location()
            .to_i32_round();

        if let Some(tree) = self.pointer_icon.prepare_dnd_icon(location) {
            elems.push(tree.into());
        }

        if let Some(tree) = self.pointer_icon.prepare_cursor_icon(location) {
            elems.push(tree.into());
        } else if let Some(texture) = pointer_image {
            elems.push(PointerElement::new(texture.clone(), location, true).into());
        }

        Ok(self
            .space
            .render_output(renderer, output, age, [0.1, 0.1, 0.1, 1.0], &elems)
            .unwrap())
    }
}
