use cgmath::{Matrix3, Vector2};
use smithay::backend::renderer::{
    gles2::{Gles2Error, Gles2Frame, Gles2Renderer, Gles2Texture},
    Frame, Transform,
};

pub struct RenderFrame<'a> {
    pub transform: Transform,
    pub frame: &'a mut Gles2Frame,

    pub renderer: &'a mut Gles2Renderer,
}

impl<'a> Frame for RenderFrame<'a> {
    type Error = Gles2Error;
    type TextureId = Gles2Texture;

    fn clear(&mut self, color: [f32; 4]) -> Result<(), Self::Error> {
        self.frame.clear(color)
    }

    fn render_texture(
        &mut self,
        texture: &Self::TextureId,
        matrix: Matrix3<f32>,
        tex_coords: [Vector2<f32>; 4],
        alpha: f32,
    ) -> Result<(), Self::Error> {
        self.frame
            .render_texture(texture, matrix, tex_coords, alpha)
    }
}
