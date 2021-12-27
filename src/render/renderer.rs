use image::{ImageBuffer, Rgba};
use smithay::{
    backend::renderer::{
        gles2::{Gles2Error, Gles2Frame, Gles2Renderer, Gles2Texture},
        Frame, Transform,
    },
    utils::{Buffer, Physical,  Rectangle},
};

pub struct RenderFrame<'a> {
    pub frame: &'a mut Gles2Frame,
    pub imgui: &'a imgui::Ui<'a>,

    pub renderer: &'a mut Gles2Renderer,
}

impl<'a> Frame for RenderFrame<'a> {
    type Error = Gles2Error;
    type TextureId = Gles2Texture;

    fn clear(
        &mut self,
        color: [f32; 4],
        at: &[Rectangle<i32, Physical>],
    ) -> Result<(), Self::Error> {
        self.frame.clear(color, at)
    }

    fn render_texture_from_to(
        &mut self,
        texture: &Self::TextureId,
        src: Rectangle<i32, Buffer>,
        dst: Rectangle<f64, Physical>,
        damage: &[Rectangle<i32, Physical>],
        src_transform: Transform,
        alpha: f32,
    ) -> Result<(), Self::Error> {
        self.frame
            .render_texture_from_to(texture, src, dst, damage, src_transform, alpha)
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
