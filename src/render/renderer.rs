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

        Self {
            inner,
            glow,
            quad_pipeline,
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
        self.inner
            .gles_renderer()
            .render(size, transform, |renderer, frame| {
                renderer
                    .with_context(|renderer, gles| {
                        func(&mut RenderFrame {
                            transform,
                            frame,
                            context: RenderContext { renderer, gles, glow },
                            quad_pipeline,
                        })
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
        self.inner.render(|renderer, frame| {
            renderer
                .with_context(|renderer, gles| {
                    func(&mut RenderFrame {
                        transform: Transform::Normal,
                        frame,
                        context: RenderContext { renderer, gles, glow },
                        quad_pipeline,
                    })
                })
                .unwrap()
        })
    }
}
