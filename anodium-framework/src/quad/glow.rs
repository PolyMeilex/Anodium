use std::{ffi::CString, num::NonZeroU32};

use smithay::backend::renderer::gles2::ffi::{
    types as native_gl, Gles2, COMPILE_STATUS, INFO_LOG_LENGTH, LINK_STATUS,
};

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Shader(pub NonZeroU32);

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Program(pub NonZeroU32);

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Buffer(pub NonZeroU32);

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct VertexArray(pub NonZeroU32);

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Texture(pub NonZeroU32);

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Sampler(pub NonZeroU32);

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Fence(pub native_gl::GLsync);

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Framebuffer(pub NonZeroU32);

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Renderbuffer(pub NonZeroU32);

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Query(pub NonZeroU32);

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct UniformLocation(pub native_gl::GLuint);

#[derive(Copy, Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TransformFeedback(pub NonZeroU32);

pub unsafe fn get_uniform_location(
    gl: &Gles2,
    program: Program,
    name: &str,
) -> Option<UniformLocation> {
    let name = CString::new(name).unwrap();
    let uniform_location =
        gl.GetUniformLocation(program.0.get(), name.as_ptr() as *const native_gl::GLchar);
    if uniform_location < 0 {
        None
    } else {
        Some(UniformLocation(uniform_location as u32))
    }
}

pub unsafe fn get_attrib_location(gl: &Gles2, program: Program, name: &str) -> Option<u32> {
    let name = CString::new(name).unwrap();
    let attrib_location =
        gl.GetAttribLocation(program.0.get(), name.as_ptr() as *const native_gl::GLchar);
    if attrib_location < 0 {
        None
    } else {
        Some(attrib_location as u32)
    }
}

pub unsafe fn get_shader_compile_status(gl: &Gles2, shader: Shader) -> bool {
    let mut status = 0;
    gl.GetShaderiv(shader.0.get(), COMPILE_STATUS, &mut status);
    1 == status
}

pub unsafe fn get_shader_info_log(gl: &Gles2, shader: Shader) -> String {
    let mut length = 0;
    gl.GetShaderiv(shader.0.get(), INFO_LOG_LENGTH, &mut length);
    if length > 0 {
        let mut log = String::with_capacity(length as usize);
        log.extend(std::iter::repeat('\0').take(length as usize));
        gl.GetShaderInfoLog(
            shader.0.get(),
            length,
            &mut length,
            (&log[..]).as_ptr() as *mut native_gl::GLchar,
        );
        log.truncate(length as usize);
        log
    } else {
        String::from("")
    }
}

pub unsafe fn get_program_link_status(gl: &Gles2, program: Program) -> bool {
    let mut status = 0;
    gl.GetProgramiv(program.0.get(), LINK_STATUS, &mut status);
    1 == status
}

pub unsafe fn get_program_info_log(gl: &Gles2, program: Program) -> String {
    let mut length = 0;
    gl.GetProgramiv(program.0.get(), INFO_LOG_LENGTH, &mut length);
    if length > 0 {
        let mut log = String::with_capacity(length as usize);
        log.extend(std::iter::repeat('\0').take(length as usize));
        gl.GetProgramInfoLog(
            program.0.get(),
            length,
            &mut length,
            (&log[..]).as_ptr() as *mut native_gl::GLchar,
        );
        log.truncate(length as usize);
        log
    } else {
        String::from("")
    }
}
