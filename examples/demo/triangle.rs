// Draws a simple white triangle
// based on the example from:
// https://github.com/brendanzab/gl-rs/blob/master/gl/examples/triangle.rs

use egui_glfw_gl::gl;
use egui_glfw_gl::gl::types::*;
use std::{mem, ptr, str};

#[allow(unconditional_panic)]
const fn illegal_null_in_string() {
    [][0]
}

#[doc(hidden)]
pub const fn validate_cstr_contents(bytes: &[u8]) {
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\0' {
            illegal_null_in_string();
        }
        i += 1;
    }
}

macro_rules! cstr {
    ( $s:literal ) => {{
        validate_cstr_contents($s.as_bytes());
        unsafe { std::mem::transmute::<_, &std::ffi::CStr>(concat!($s, "\0")) }
    }};
}

const VS_SRC: &str = "
#version 150
in vec2 position;

void main() {
    gl_Position = vec4(position, 0.0, 1.0);
}";

const FS_SRC: &str = "
#version 150
out vec4 out_color;

void main() {
    out_color = vec4(1.0, 1.0, 1.0, 1.0);
}";

static VERTEX_DATA: [GLfloat; 6] = [0.0, 0.5, 0.5, -0.5, -0.5, -0.5];

pub struct Triangle {
    pub vs: GLuint,
    pub fs: GLuint,
    pub program: GLuint,
    pub vao: GLuint,
    pub vbo: GLuint,
}

impl Triangle {
    pub fn new() -> Self {
        let vs = egui_glfw_gl::painter::compile_shader(VS_SRC, gl::VERTEX_SHADER);
        let fs = egui_glfw_gl::painter::compile_shader(FS_SRC, gl::FRAGMENT_SHADER);
        let program = egui_glfw_gl::painter::link_program(vs, fs);

        let mut vao = 0;
        let mut vbo = 0;
        unsafe {
            gl::GenVertexArrays(1, &mut vao);
            gl::GenBuffers(1, &mut vbo);
        }

        Triangle {
            vs,
            fs,
            program,
            vao,
            vbo,
        }
    }

    pub fn draw(&self) {
        unsafe {
            gl::BindVertexArray(self.vao);

            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (VERTEX_DATA.len() * mem::size_of::<GLfloat>()) as GLsizeiptr,
                mem::transmute(&VERTEX_DATA[0]),
                gl::STATIC_DRAW,
            );
        }

        unsafe {
            gl::UseProgram(self.program);
        }

        let c_out_color = cstr!("out_color");
        unsafe {
            gl::BindFragDataLocation(self.program, 0, c_out_color.as_ptr());
        }

        let c_position = cstr!("position");
        let pos_attr = unsafe { gl::GetAttribLocation(self.program, c_position.as_ptr()) };
        unsafe {
            gl::EnableVertexAttribArray(pos_attr as GLuint);
            gl::VertexAttribPointer(
                pos_attr as GLuint,
                2,
                gl::FLOAT,
                gl::FALSE as GLboolean,
                0,
                ptr::null(),
            );
        }

        unsafe {
            gl::DrawArrays(gl::TRIANGLES, 0, 3);
        }
    }
}

impl Drop for Triangle {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.program);
            gl::DeleteShader(self.fs);
            gl::DeleteShader(self.vs);
            gl::DeleteBuffers(1, &self.vbo);
            gl::DeleteVertexArrays(1, &self.vao);
        }
    }
}
