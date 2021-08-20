use cgmath::Matrix3;
use glow::HasContext;
use smithay::{
    backend::renderer::{gles2::ffi, Transform},
    utils::{Physical, Rectangle},
};

use super::renderer::RenderContext;

pub struct QuadPipeline {
    program: glow::Program,

    projection: glow::UniformLocation,
    color: glow::UniformLocation,
    position: u32, // AtributeLocation,
}

impl QuadPipeline {
    pub fn new(context: &mut RenderContext) -> Self {
        let program = create_program(
            context,
            include_str!("./shaders/quad.vert"),
            include_str!("./shaders/quad.frag"),
        );

        let gl = context.glow;

        let (projection, color, position) = unsafe {
            (
                gl.get_uniform_location(program, "projection").unwrap(),
                gl.get_uniform_location(program, "color").unwrap(),
                gl.get_attrib_location(program, "position").unwrap(),
            )
        };

        Self {
            program,

            projection,
            position,
            color,
        }
    }

    pub fn render(
        &self,
        output_geometry: Rectangle<f64, Physical>,
        mut quad_rect: Rectangle<f64, Physical>,
        transform: Transform,
        context: &RenderContext,
        alpha: f32,
    ) {
        quad_rect.loc.x -= output_geometry.loc.x;

        let glow = context.glow;
        let gles = context.gles;

        let screen = Matrix3 {
            x: [2.0 / output_geometry.size.w as f32, 0.0, 0.0].into(),
            y: [0.0, -2.0 / output_geometry.size.h as f32, 0.0].into(),
            z: [-1.0, 1.0, 1.0].into(),
        };

        let x = quad_rect.loc.x as f32;
        let y = quad_rect.loc.y as f32;

        let w = quad_rect.size.w as f32;
        let h = quad_rect.size.h as f32;

        let quad = Matrix3 {
            x: [w, 0.0, 0.0].into(),
            y: [0.0, h, 0.0].into(),
            z: [x, y, 1.0].into(),
        };

        unsafe {
            glow.use_program(Some(self.program));

            let mat = transform.matrix() * screen * quad;
            let mat: &[f32; 9] = mat.as_ref();

            glow.uniform_matrix_3_f32_slice(Some(&self.projection), false, mat);

            glow.uniform_4_f32(
                Some(&self.color),
                26.0 / 255.0,
                95.0 / 255.0,
                205.0 / 255.0,
                alpha,
            );

            gles.VertexAttribPointer(
                self.position,
                2,
                ffi::FLOAT,
                ffi::FALSE,
                0,
                VERTS.as_ptr() as *const _,
            );

            glow.enable_vertex_attrib_array(self.position);

            glow.draw_arrays(glow::TRIANGLE_STRIP, 0, 4);

            glow.disable_vertex_attrib_array(self.position);
            glow.use_program(None);
        }
    }
}

static VERTS: [ffi::types::GLfloat; 8] = [
    1.0, 0.0, // top right
    0.0, 0.0, // top left
    1.0, 1.0, // bottom right
    0.0, 1.0, // bottom left
];

fn create_program(
    context: &mut RenderContext,
    vertex_shader_source: &str,
    fragment_shader_source: &str,
) -> glow::Program {
    let gl = &context.glow;
    unsafe {
        let program = gl.create_program().expect("Cannot create program");

        let shader_sources = [
            (glow::VERTEX_SHADER, vertex_shader_source),
            (glow::FRAGMENT_SHADER, fragment_shader_source),
        ];

        let mut shaders = Vec::with_capacity(shader_sources.len());

        for (shader_type, shader_source) in shader_sources.iter() {
            let shader = gl.create_shader(*shader_type).expect("Cannot create shader");
            gl.shader_source(shader, shader_source);
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                panic!("{}", gl.get_shader_info_log(shader));
            }
            gl.attach_shader(program, shader);
            shaders.push(shader);
        }

        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            panic!("{}", gl.get_program_info_log(program));
        }

        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }

        program
    }
}
