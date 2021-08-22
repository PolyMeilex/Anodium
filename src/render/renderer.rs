#![allow(dead_code)]

use std::ops::{Deref, DerefMut};

use cgmath::{Matrix3, Vector2};
use smithay::{
    backend::{
        renderer::{
            gles2::{ffi::Gles2, Gles2Error, Gles2Frame, Gles2Renderer, Gles2Texture},
            Frame, Transform,
        },
        winit::WinitGraphicsBackend,
        SwapBuffersError,
    },
    utils::{Physical, Size},
};

use super::QuadPipeline;
pub trait HasGles2Renderer {
    fn gles_renderer(&mut self) -> &mut Gles2Renderer;
}

pub struct AnodiumRenderer<B> {
    inner: B,
    glow: glow::Context,
    quad_pipeline: QuadPipeline,

    #[cfg(feature = "debug")]
    imgui_pipeline: imgui_smithay_renderer::Renderer,
    #[cfg(feature = "debug")]
    imgui: imgui::Context,
}

impl<B: HasGles2Renderer> AnodiumRenderer<B> {
    pub fn new(mut inner: B) -> Self {
        let glow = unsafe {
            glow::Context::from_loader_function(|symbol| smithay::backend::egl::get_proc_address(symbol))
        };

        let quad_pipeline = inner
            .gles_renderer()
            .with_context(|renderer, gles| {
                QuadPipeline::new(&mut RenderContext {
                    renderer,
                    gles,
                    glow: &glow,
                })
            })
            .unwrap();

        #[cfg(feature = "debug")]
        let mut imgui = imgui::Context::create();

        #[cfg(feature = "debug")]
        {
            imgui.set_ini_filename(None);
            let io = imgui.io_mut();
            let hidpi_factor = 1.0;
            io.display_framebuffer_scale = [hidpi_factor as f32, hidpi_factor as f32];
            let logical_size = (1920, 1080);
            io.display_size = [logical_size.0 as f32, logical_size.1 as f32];
        }

        #[cfg(feature = "debug")]
        let imgui_pipeline = inner
            .gles_renderer()
            .with_context(|_, gles| imgui_smithay_renderer::Renderer::new(gles, &mut imgui))
            .unwrap();

        Self {
            inner,
            glow,
            quad_pipeline,

            #[cfg(feature = "debug")]
            imgui_pipeline,
            #[cfg(feature = "debug")]
            imgui,
        }
    }

    pub fn with_context<F, R>(&mut self, func: F) -> Result<R, Gles2Error>
    where
        F: FnOnce(&mut RenderContext) -> R,
    {
        let glow = &self.glow;
        self.inner
            .gles_renderer()
            .with_context(|renderer, gles| func(&mut RenderContext { renderer, gles, glow }))
    }

    pub fn render<F, R>(
        &mut self,
        size: Size<i32, Physical>,
        transform: Transform,
        func: F,
    ) -> Result<R, Gles2Error>
    where
        F: FnOnce(&mut RenderFrame) -> R,
    {
        use smithay::backend::renderer::Renderer;
        let glow = &self.glow;
        let quad_pipeline = &self.quad_pipeline;

        #[cfg(feature = "debug")]
        let imgui = &mut self.imgui;
        #[cfg(feature = "debug")]
        let imgui_pipeline = &self.imgui_pipeline;

        self.inner
            .gles_renderer()
            .render(size, transform, |renderer, frame| {
                renderer
                    .with_context(|renderer, gles| {
                        #[cfg(feature = "debug")]
                        let imgui_frame = imgui.frame();

                        let ret = func(&mut RenderFrame {
                            transform,
                            frame,
                            context: RenderContext { renderer, gles, glow },

                            quad_pipeline,

                            #[cfg(feature = "debug")]
                            imgui_frame: &imgui_frame,
                        });

                        #[cfg(feature = "debug")]
                        {
                            let draw_data = imgui_frame.render();
                            imgui_pipeline.render(transform, gles, draw_data);
                        }

                        ret
                    })
                    .unwrap()
            })
    }
}

impl<B> Deref for AnodiumRenderer<B> {
    type Target = B;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl<B> DerefMut for AnodiumRenderer<B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

pub struct RenderContext<'a> {
    pub renderer: &'a mut Gles2Renderer,
    pub gles: &'a Gles2,
    pub glow: &'a glow::Context,
}

pub struct RenderFrame<'a> {
    pub transform: Transform,
    pub frame: &'a mut Gles2Frame,

    pub context: RenderContext<'a>,

    pub quad_pipeline: &'a QuadPipeline,

    #[cfg(feature = "debug")]
    pub imgui_frame: &'a imgui::Ui<'a>,
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
        self.frame.render_texture(texture, matrix, tex_coords, alpha)
    }
}

impl HasGles2Renderer for WinitGraphicsBackend {
    fn gles_renderer(&mut self) -> &mut Gles2Renderer {
        self.renderer()
    }
}

impl HasGles2Renderer for Gles2Renderer {
    fn gles_renderer(&mut self) -> &mut Gles2Renderer {
        self
    }
}

impl AnodiumRenderer<WinitGraphicsBackend> {
    pub fn render_winit<F, R>(&mut self, func: F) -> Result<R, SwapBuffersError>
    where
        F: FnOnce(&mut RenderFrame) -> R,
    {
        let glow = &self.glow;
        let quad_pipeline = &self.quad_pipeline;

        #[cfg(feature = "debug")]
        let imgui = &mut self.imgui;
        #[cfg(feature = "debug")]
        let imgui_pipeline = &self.imgui_pipeline;

        let ret = self.inner.render(|renderer, frame| {
            renderer
                .with_context(|renderer, gles| {
                    #[cfg(feature = "debug")]
                    let imgui_frame = imgui.frame();

                    let ret = func(&mut RenderFrame {
                        transform: Transform::Normal,
                        frame,
                        context: RenderContext { renderer, gles, glow },

                        quad_pipeline,
                        #[cfg(feature = "debug")]
                        imgui_frame: &imgui_frame,
                    });

                    #[cfg(feature = "debug")]
                    {
                        let draw_data = imgui_frame.render();
                        imgui_pipeline.render(Transform::Normal, gles, draw_data);
                    }

                    ret
                })
                .unwrap()
        });

        ret
    }
}
