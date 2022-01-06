use image::{ImageBuffer, Rgba};
use imgui_smithay_renderer::Renderer;
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

pub fn import_bitmap<C: std::ops::Deref<Target = [u8]>>(
    renderer: &mut Gles2Renderer,
    image: &ImageBuffer<Rgba<u8>, C>,
    scale: Option<(i32, i32)>,
) -> Result<Gles2Texture, Gles2Error> {
    use smithay::backend::renderer::gles2::ffi;

    renderer.with_context(|renderer, gl| unsafe {
        let mut tex = 0;
        gl.GenTextures(1, &mut tex);
        gl.BindTexture(ffi::TEXTURE_2D, tex);
        gl.TexParameteri(
            ffi::TEXTURE_2D,
            ffi::TEXTURE_WRAP_S,
            ffi::CLAMP_TO_EDGE as i32,
        );
        gl.TexParameteri(
            ffi::TEXTURE_2D,
            ffi::TEXTURE_WRAP_T,
            ffi::CLAMP_TO_EDGE as i32,
        );
        gl.TexImage2D(
            ffi::TEXTURE_2D,
            0,
            ffi::RGBA as i32,
            image.width() as i32,
            image.height() as i32,
            0,
            ffi::RGBA,
            ffi::UNSIGNED_BYTE as u32,
            image.as_ptr() as *const _,
        );
        gl.BindTexture(ffi::TEXTURE_2D, 0);

        let size = if let Some(scale) = scale {
            scale
        } else {
            (image.width() as i32, image.height() as i32)
        };

        Gles2Texture::from_raw(renderer, tex, size.into())
    })
}
