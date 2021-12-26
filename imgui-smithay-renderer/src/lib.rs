pub extern crate imgui;
use std::ffi::CString;

use cgmath::SquareMatrix;
use smithay::backend::renderer::gles2::ffi::Gles2;
use smithay::backend::renderer::{gles2::ffi, Transform};

use imgui::{DrawCmd, DrawCmdParams, Textures};

#[macro_use]
extern crate memoffset;

unsafe fn get_attrib_location(gl: &Gles2, program: ffi::types::GLuint, name: &str) -> Option<u32> {
    let name = CString::new(name).unwrap();
    let attrib_location = gl.GetAttribLocation(program, name.as_ptr() as *const ffi::types::GLchar);
    if attrib_location < 0 {
        None
    } else {
        Some(attrib_location as u32)
    }
}

unsafe fn get_uniform_location(gl: &Gles2, program: ffi::types::GLuint, name: &str) -> Option<u32> {
    let name = CString::new(name).unwrap();
    let uniform_location =
        gl.GetUniformLocation(program, name.as_ptr() as *const ffi::types::GLchar);
    if uniform_location < 0 {
        None
    } else {
        Some(uniform_location as u32)
    }
}

#[derive(Debug)]
pub struct Renderer {
    program: ffi::types::GLuint,
    font_texture: ffi::types::GLuint,

    ebo: ffi::types::GLuint,
    vao: ffi::types::GLuint,
    vbo: ffi::types::GLuint,

    textures: Textures<ffi::types::GLuint>,
}

impl Renderer {
    pub fn new(gl: &Gles2, imgui: &mut imgui::Context) -> Self {
        let (program, vao, vbo, ebo) = unsafe {
            let program = gl.CreateProgram();

            let (vertex_shader_source, fragment_shader_source) = (
                include_str!("./shaders/quad.vert"),
                include_str!("./shaders/quad.frag"),
            );

            let shader_sources = [
                (ffi::VERTEX_SHADER, vertex_shader_source),
                (ffi::FRAGMENT_SHADER, fragment_shader_source),
            ];

            let mut shaders = Vec::with_capacity(shader_sources.len());

            for (shader_type, shader_source) in shader_sources.iter() {
                let shader = gl.CreateShader(*shader_type);
                gl.ShaderSource(
                    shader,
                    1,
                    &(shader_source.as_ptr() as _),
                    &(shader_source.len() as _),
                );
                gl.CompileShader(shader);
                gl.AttachShader(program, shader);
                shaders.push(shader);
            }

            gl.LinkProgram(program);

            for shader in shaders {
                gl.DetachShader(program, shader);
                gl.DeleteShader(shader);
            }

            let vao = {
                let mut vao = 0;
                gl.GenVertexArrays(1, &mut vao);
                vao
            };

            let ebo = {
                let mut ebo = 0;
                gl.GenBuffers(1, &mut ebo);
                ebo
            };
            let vbo = {
                let mut vbo = 0;
                gl.GenBuffers(1, &mut vbo);
                vbo
            };

            (program, vao, vbo, ebo)
        };

        let font_texture = unsafe {
            // Build fonts atlas

            let font_texture = {
                let mut font_texture = 0;
                gl.GenTextures(1, &mut font_texture);
                font_texture
            };
            gl.BindTexture(ffi::TEXTURE_2D, font_texture);

            gl.TexParameteri(ffi::TEXTURE_2D, ffi::TEXTURE_MIN_FILTER, ffi::LINEAR as _);
            gl.TexParameteri(ffi::TEXTURE_2D, ffi::TEXTURE_MAG_FILTER, ffi::LINEAR as _);

            let mut fonts = imgui.fonts();
            let texture_atlas = fonts.build_rgba32_texture();
            gl.TexImage2D(
                ffi::TEXTURE_2D,
                0,
                ffi::RGBA as _,
                texture_atlas.width as _,
                texture_atlas.height as _,
                0,
                ffi::RGBA,
                ffi::UNSIGNED_BYTE,
                texture_atlas.data.as_ptr() as *const _,
            );
            gl.PixelStorei(ffi::UNPACK_ROW_LENGTH, 0);

            fonts.tex_id = imgui::TextureId::from(usize::MAX);
            font_texture
        };

        let mut textures_hashmap = Textures::new();
        textures_hashmap.replace(imgui.fonts().tex_id, font_texture);

        let mut renderer = Renderer {
            program,
            font_texture,
            ebo,
            vao,
            vbo,

            textures: textures_hashmap,
        };

        renderer.setup(gl, imgui);

        renderer
    }

    fn setup(&mut self, gl: &Gles2, _: &mut imgui::Context) {
        unsafe {
            gl.BindVertexArray(self.vao);
            gl.BindBuffer(ffi::ARRAY_BUFFER, self.vbo);

            let pos_attrib_loc =
                get_attrib_location(gl, self.program, "pos").expect("could not find pos attrib");
            let uv_attrib_loc =
                get_attrib_location(gl, self.program, "uv").expect("could not find uv attrib");
            let color_attrib_loc =
                get_attrib_location(gl, self.program, "col").expect("could not find color attrib");

            gl.EnableVertexAttribArray(pos_attrib_loc);
            gl.VertexAttribPointer(
                pos_attrib_loc,
                2,
                ffi::FLOAT,
                false as u8,
                std::mem::size_of::<imgui::DrawVert>() as i32,
                offset_of!(imgui::DrawVert, pos) as i32 as *const std::ffi::c_void,
            );

            gl.EnableVertexAttribArray(uv_attrib_loc);
            gl.VertexAttribPointer(
                uv_attrib_loc,
                2,
                ffi::FLOAT,
                false as u8,
                std::mem::size_of::<imgui::DrawVert>() as i32,
                offset_of!(imgui::DrawVert, uv) as i32 as *const std::ffi::c_void,
            );

            gl.EnableVertexAttribArray(color_attrib_loc);
            gl.VertexAttribPointer(
                color_attrib_loc,
                4,
                ffi::UNSIGNED_BYTE,
                true as u8,
                std::mem::size_of::<imgui::DrawVert>() as i32,
                offset_of!(imgui::DrawVert, col) as i32 as *const std::ffi::c_void,
            );

            gl.BindVertexArray(0);
            gl.BindBuffer(ffi::ELEMENT_ARRAY_BUFFER, 0);
            gl.BindBuffer(ffi::ARRAY_BUFFER, 0);
        }
    }

    pub fn render(&self, transform: Transform, gl: &Gles2, draw_data: &imgui::DrawData) {
        let fb_width = draw_data.display_size[0] * draw_data.framebuffer_scale[0];
        let fb_height = draw_data.display_size[1] * draw_data.framebuffer_scale[1];
        if !(fb_width > 0.0 && fb_height > 0.0) {
            return;
        }

        let left = draw_data.display_pos[0];
        let right = draw_data.display_pos[0] + draw_data.display_size[0];
        let top = draw_data.display_pos[1];
        let bottom = draw_data.display_pos[1] + draw_data.display_size[1];

        let matrix = cgmath::Matrix4 {
            x: ((2.0 / (right - left)), 0.0, 0.0, 0.0).into(),
            y: (0.0, (2.0 / (top - bottom)), 0.0, 0.0).into(),
            z: (0.0, 0.0, -1.0, 0.0).into(),
            w: (
                (right + left) / (left - right),
                (top + bottom) / (bottom - top),
                0.0,
                1.0,
            )
                .into(),
        };

        let transform3 = transform.matrix();
        let mut transform = cgmath::Matrix4::identity();

        {
            let mut row = &mut transform.x;
            let row3 = &transform3.x;
            row.x = row3.x;
            row.y = row3.y;
            row.z = row3.z;
        }
        {
            let mut row = &mut transform.y;
            let row3 = &transform3.y;
            row.x = row3.x;
            row.y = row3.y;
            row.z = row3.z;
        }
        {
            let mut row = &mut transform.z;
            let row3 = &transform3.z;
            row.x = row3.x;
            row.y = row3.y;
            row.z = row3.z;
        }

        let matrix_src = transform * matrix;

        let matrix: &[f32; 16] = matrix_src.as_ref();

        // let clip_off = draw_data.display_pos;
        // let clip_scale = draw_data.framebuffer_scale;

        unsafe {
            gl.BindVertexArray(self.vao);
            gl.BindBuffer(ffi::ARRAY_BUFFER, self.vbo);

            for draw_list in draw_data.draw_lists() {
                {
                    let vtx_buffer = draw_list.vtx_buffer();
                    let buffer = std::slice::from_raw_parts(
                        vtx_buffer.as_ptr() as *const u8,
                        vtx_buffer.len() * std::mem::size_of::<imgui::DrawVert>(),
                    );

                    let mut indices: Vec<u16> = Vec::new();

                    for data in draw_list.idx_buffer() {
                        indices.push(*data);
                    }

                    gl.BufferData(
                        ffi::ARRAY_BUFFER,
                        buffer.len() as isize,
                        buffer.as_ptr() as *const _,
                        ffi::STREAM_DRAW,
                    );

                    gl.BindBuffer(ffi::ELEMENT_ARRAY_BUFFER, self.ebo);

                    let slice = std::slice::from_raw_parts(
                        indices.as_ptr() as *const u8,
                        indices.len() * std::mem::size_of::<u16>(),
                    );
                    gl.BufferData(
                        ffi::ELEMENT_ARRAY_BUFFER,
                        slice.len() as isize,
                        slice.as_ptr() as *const _,
                        ffi::STREAM_DRAW,
                    );
                }

                gl.Enable(ffi::BLEND);
                gl.BlendFunc(ffi::SRC_ALPHA, ffi::ONE_MINUS_SRC_ALPHA);
                gl.UseProgram(self.program);

                let shader_loc = get_uniform_location(gl, self.program, "matrix")
                    .expect("error finding matrix uniform");

                gl.UniformMatrix4fv(
                    shader_loc as i32,
                    matrix.len() as i32 / 16,
                    false as u8,
                    matrix.as_ptr(),
                );

                let texture_loc = get_uniform_location(gl, self.program, "tex")
                    .expect("error finding texture sampler uniform");
                gl.Uniform1i(texture_loc as i32, 0);

                gl.Enable(ffi::SCISSOR_TEST);
                gl.BindVertexArray(self.vao);
                for cmd in draw_list.commands() {
                    match cmd {
                        DrawCmd::Elements {
                            count,
                            cmd_params:
                                DrawCmdParams {
                                    // clip_rect,
                                    idx_offset,
                                    texture_id,
                                    ..
                                },
                        } => {
                            // let x = (clip_rect[0] - clip_off[0]) * clip_scale[0];
                            // let y = (clip_rect[1] - clip_off[1]) * clip_scale[1];
                            // let z = (clip_rect[2] - clip_off[0]) * clip_scale[0];
                            // let w = (clip_rect[3] - clip_off[1]) * clip_scale[1];

                            let texture = self.textures.get(texture_id).unwrap();
                            gl.BindTexture(ffi::TEXTURE_2D, *texture);

                            // gl.Scissor(x as _, (fb_height - w) as _, (z - x) as _, (w - y) as _);
                            gl.DrawElements(
                                ffi::TRIANGLES,
                                count as i32,
                                ffi::UNSIGNED_SHORT,
                                (idx_offset * std::mem::size_of::<u16>()) as _,
                            );
                        }

                        _ => (),
                    }
                }

                gl.Disable(ffi::SCISSOR_TEST);
                gl.BindVertexArray(0);
                gl.BindTexture(ffi::TEXTURE_2D, 0);
            }

            gl.BindVertexArray(0);
            gl.BindBuffer(ffi::ARRAY_BUFFER, 0);
        }
    }

    pub fn cleanup(&self, gl: &Gles2) {
        unsafe {
            gl.DeleteBuffers(1, &self.vbo);
            gl.DeleteBuffers(1, &self.ebo);
            gl.DeleteVertexArrays(1, &self.vao);
            gl.DeleteTextures(1, &self.font_texture);
            gl.DeleteProgram(self.program);
        }
    }
}
