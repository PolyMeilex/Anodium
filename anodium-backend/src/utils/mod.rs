use image::{ImageBuffer, Rgba};
use smithay::backend::renderer::gles2::{Gles2Error, Gles2Renderer, Gles2Texture};

// pub mod cursor;

pub fn import_bitmap<C: std::ops::Deref<Target = [u8]>>(
    renderer: &mut Gles2Renderer,
    image: &ImageBuffer<Rgba<u8>, C>,
    scale: Option<(i32, i32)>,
) -> Result<Gles2Texture, Gles2Error> {
    use smithay::backend::renderer::gles2::ffi;

    let (tex, size) = renderer.with_context(|gl| unsafe {
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

        (tex, size)
    })?;

    let tex = unsafe { Gles2Texture::from_raw(renderer, tex, size.into()) };

    Ok(tex)
}
