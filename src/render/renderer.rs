use smithay::{
    backend::renderer::{
        gles2::{Gles2Error, Gles2Frame, Gles2Renderer, Gles2Texture},
        Frame, Transform,
    },
    utils::{Buffer, Physical, Point, Rectangle},
};

pub struct RenderFrame<'a> {
    pub transform: Transform,
    pub frame: &'a mut Gles2Frame,
    pub imgui: &'a imgui::Ui<'a>,

    pub renderer: &'a mut Gles2Renderer,
}

impl<'a> Frame for RenderFrame<'a> {
    type Error = Gles2Error;
    type TextureId = Gles2Texture;

    fn clear(&mut self, color: [f32; 4]) -> Result<(), Self::Error> {
        self.frame.clear(color)
    }

    fn render_texture_at(
        &mut self,
        texture: &Self::TextureId,
        pos: Point<f64, Physical>,
        texture_scale: i32,
        output_scale: f64,
        src_transform: Transform,
        alpha: f32,
    ) -> Result<(), Self::Error> {
        self.frame.render_texture_at(
            texture,
            pos,
            texture_scale,
            output_scale,
            src_transform,
            alpha,
        )
    }

    fn render_texture_from_to(
        &mut self,
        texture: &Self::TextureId,
        src: Rectangle<i32, Buffer>,
        dst: Rectangle<f64, Physical>,
        src_transform: Transform,
        alpha: f32,
    ) -> Result<(), Self::Error> {
        self.frame
            .render_texture_from_to(texture, src, dst, src_transform, alpha)
    }
}
