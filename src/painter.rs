extern crate gl;

use egui::{
    emath::Rect,
    epaint::{Mesh, Primitive},
    Color32, TextureFilter,
};

use gl::types::{GLchar, GLenum, GLint, GLsizeiptr, GLuint};
use std::ffi::{c_void, CString};

fn compile_shader(src: &str, ty: GLenum) -> GLuint {
    let shader = unsafe { gl::CreateShader(ty) };

    let c_str = CString::new(src.as_bytes()).unwrap();
    unsafe {
        gl::ShaderSource(shader, 1, &c_str.as_ptr(), core::ptr::null());
        gl::CompileShader(shader);
    }

    let mut status = gl::FALSE as GLint;
    unsafe {
        gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut status);
    }

    if status != (gl::TRUE as GLint) {
        let mut len = 0;
        unsafe {
            gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut len);
        }

        let mut buf = vec![0; len as usize];

        unsafe {
            gl::GetShaderInfoLog(
                shader,
                len,
                core::ptr::null_mut(),
                buf.as_mut_ptr() as *mut GLchar,
            );
        }

        panic!(
            "{}",
            core::str::from_utf8(&buf).expect("ShaderInfoLog not valid utf8")
        );
    }

    shader
}

fn link_program(vs: GLuint, fs: GLuint) -> GLuint {
    let program = unsafe { gl::CreateProgram() };

    unsafe {
        gl::AttachShader(program, vs);
        gl::AttachShader(program, fs);
        gl::LinkProgram(program);
    }

    let mut status = gl::FALSE as GLint;
    unsafe {
        gl::GetProgramiv(program, gl::LINK_STATUS, &mut status);
    }

    if status != (gl::TRUE as GLint) {
        let mut len: GLint = 0;
        unsafe {
            gl::GetProgramiv(program, gl::INFO_LOG_LENGTH, &mut len);
        }

        let mut buf = vec![0; len as usize];

        unsafe {
            gl::GetProgramInfoLog(
                program,
                len,
                core::ptr::null_mut(),
                buf.as_mut_ptr() as *mut GLchar,
            );
        }

        panic!(
            "{}",
            core::str::from_utf8(&buf).expect("ProgramInfoLog not valid utf8")
        );
    }

    program
}

#[derive(Default)]
pub struct UserTexture {
    size: (usize, usize),

    /// Pending upload (will be emptied later).
    pixels: Vec<u8>,

    /// Lazily uploaded
    texture: Option<GLuint>,

    /// For user textures there is a choice between
    /// Linear (default) and Nearest.
    filtering: TextureFilter,

    /// User textures can be modified and this flag
    /// is used to indicate if pixel data for the
    /// texture has been updated.
    dirty: bool,
}

impl UserTexture {
    pub fn update_texture_part(
        &mut self,
        x_offset: i32,
        y_offset: i32,
        width: i32,
        height: i32,
        bytes: &[u8],
    ) {
        assert!(x_offset + width <= self.size.0 as _);
        assert!(y_offset + height <= self.size.1 as _);

        unsafe {
            gl::PixelStorei(gl::UNPACK_ALIGNMENT, 1);
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_SWIZZLE_A, gl::RED as _);

            gl::TexSubImage2D(
                gl::TEXTURE_2D,
                0,
                x_offset as _,
                y_offset as _,
                width as _,
                height as _,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                bytes.as_ptr() as *const _,
            );
        }

        self.dirty = true;
    }

    pub fn from_raw(id: u32) -> Self {
        Self {
            size: (0, 0),
            texture: Some(id),
            filtering: TextureFilter::Linear,
            dirty: false,
            pixels: Vec::with_capacity(0),
        }
    }

    pub fn delete(&self) {
        if let Some(texture) = &self.texture {
            unsafe {
                gl::DeleteTextures(1, texture as *const _);
            }
        }
    }
}

pub struct Painter {
    program: GLuint,

    vertex_array: GLuint,
    index_buffer: GLuint,
    pos_buffer: GLuint,
    tc_buffer: GLuint,
    color_buffer: GLuint,

    canvas_width: u32,
    canvas_height: u32,

    textures: std::collections::HashMap<egui::TextureId, UserTexture>,
}

impl Painter {
    pub fn new(window: &mut glfw::Window) -> Painter {
        let vs = compile_shader(include_str!("shader/vertex.vert"), gl::VERTEX_SHADER);
        let fs = compile_shader(include_str!("shader/fragment.frag"), gl::FRAGMENT_SHADER);

        let program = link_program(vs, fs);

        let mut vertex_array = 0;
        let mut index_buffer = 0;
        let mut pos_buffer = 0;
        let mut tc_buffer = 0;
        let mut color_buffer = 0;
        unsafe {
            gl::GenVertexArrays(1, &mut vertex_array);
            gl::BindVertexArray(vertex_array);
            gl::GenBuffers(1, &mut index_buffer);
            gl::GenBuffers(1, &mut pos_buffer);
            gl::GenBuffers(1, &mut tc_buffer);
            gl::GenBuffers(1, &mut color_buffer);
        }

        let (canvas_width, canvas_height) = window.get_size();

        Painter {
            program,

            vertex_array,
            index_buffer,
            pos_buffer,
            tc_buffer,
            color_buffer,

            canvas_width: canvas_width as _,
            canvas_height: canvas_height as _,

            textures: Default::default(),
        }
    }

    pub fn paint_and_update_textures(
        &mut self,
        pixels_per_point: f32,
        clipped_primitives: &[egui::ClippedPrimitive],
        textures_delta: &egui::TexturesDelta,
    ) {
        for (id, image_delta) in &textures_delta.set {
            self.set_texture(*id, image_delta);
        }

        self.paint_primitives(pixels_per_point, clipped_primitives);

        for &id in &textures_delta.free {
            self.free_texture(id);
        }
    }

    /// Main entry-point for painting a frame.
    pub fn paint_primitives(
        &mut self,
        pixels_per_point: f32,
        clipped_primitives: &[egui::ClippedPrimitive],
    ) {
        self.upload_user_textures();

        unsafe {
            //Let OpenGL know we are dealing with SRGB colors so that it
            //can do the blending correctly. Not setting the framebuffer
            //leads to darkened, oversaturated colors.
            gl::Enable(gl::FRAMEBUFFER_SRGB);

            gl::Enable(gl::SCISSOR_TEST);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::ONE, gl::ONE_MINUS_SRC_ALPHA); // premultiplied alpha
            gl::UseProgram(self.program);
            gl::ActiveTexture(gl::TEXTURE0);
        }

        let u_screen_size = CString::new("u_screen_size").unwrap();
        let u_screen_size_ptr = u_screen_size.as_ptr();
        let u_screen_size_loc = unsafe { gl::GetUniformLocation(self.program, u_screen_size_ptr) };
        let screen_size_pixels = egui::vec2(self.canvas_width as f32, self.canvas_height as f32);
        let screen_size_points = screen_size_pixels / pixels_per_point;

        unsafe {
            gl::Uniform2f(
                u_screen_size_loc,
                screen_size_points.x,
                screen_size_points.y,
            );
        }

        let u_sampler = CString::new("u_sampler").unwrap();
        let u_sampler_ptr = u_sampler.as_ptr();
        let u_sampler_loc = unsafe { gl::GetUniformLocation(self.program, u_sampler_ptr) };
        unsafe {
            gl::Uniform1i(u_sampler_loc, 0);
            gl::Viewport(0, 0, self.canvas_width as i32, self.canvas_height as i32);
        }

        for egui::ClippedPrimitive {
            clip_rect,
            primitive,
        } in clipped_primitives
        {
            match primitive {
                Primitive::Mesh(mesh) => {
                    self.paint_mesh(mesh, clip_rect, pixels_per_point);
                    unsafe {
                        gl::Disable(gl::SCISSOR_TEST);
                    }
                }

                Primitive::Callback(_) => {
                    panic!("Custom rendering callbacks are not implemented in egui_glium");
                }
            }
        }

        unsafe {
            gl::Disable(gl::FRAMEBUFFER_SRGB);
        }
    }

    pub fn new_user_texture(
        &mut self,
        size: (usize, usize),
        srgba_pixels: &[Color32],
        filtering: TextureFilter,
    ) -> egui::TextureId {
        assert_eq!(size.0 * size.1, srgba_pixels.len());

        let pixels: Vec<u8> = srgba_pixels.iter().flat_map(|a| a.to_array()).collect();
        let id = egui::TextureId::User(self.textures.len() as u64);

        self.textures.insert(
            id,
            UserTexture {
                size,
                pixels,
                texture: None,
                filtering,
                dirty: true,
            },
        );

        id
    }

    pub fn update_user_texture_data(&mut self, texture_id: &egui::TextureId, pixels: &[Color32]) {
        let texture = self
            .textures
            .get_mut(texture_id)
            .expect("Texture with id has not been created");

        texture.pixels = pixels.iter().flat_map(|a| a.to_array()).collect();
        texture.dirty = true;
    }

    fn paint_mesh(&self, mesh: &Mesh, clip_rect: &Rect, pixels_per_point: f32) {
        debug_assert!(mesh.is_valid());

        let it = self
            .textures
            .get(&mesh.texture_id)
            .expect("Textures should have been added to hash map by now");

        unsafe {
            gl::BindTexture(
                gl::TEXTURE_2D,
                it.texture
                    .expect("Texture should have a valid OpenGL id now"),
            );
        }

        let screen_size_pixels = egui::vec2(self.canvas_width as f32, self.canvas_height as f32);

        let clip_min_x = pixels_per_point * clip_rect.min.x;
        let clip_min_y = pixels_per_point * clip_rect.min.y;
        let clip_max_x = pixels_per_point * clip_rect.max.x;
        let clip_max_y = pixels_per_point * clip_rect.max.y;
        let clip_min_x = clip_min_x.clamp(0.0, screen_size_pixels.x);
        let clip_min_y = clip_min_y.clamp(0.0, screen_size_pixels.y);
        let clip_max_x = clip_max_x.clamp(clip_min_x, screen_size_pixels.x);
        let clip_max_y = clip_max_y.clamp(clip_min_y, screen_size_pixels.y);
        let clip_min_x = clip_min_x.round() as i32;
        let clip_min_y = clip_min_y.round() as i32;
        let clip_max_x = clip_max_x.round() as i32;
        let clip_max_y = clip_max_y.round() as i32;

        //scissor Y coordinate is from the bottom
        unsafe {
            gl::Scissor(
                clip_min_x,
                self.canvas_height as i32 - clip_max_y,
                clip_max_x - clip_min_x,
                clip_max_y - clip_min_y,
            );
        }

        let indices: Vec<u16> = mesh.indices.iter().map(move |idx| *idx as u16).collect();
        let indices_len = indices.len();
        let vertices_len = mesh.vertices.len();

        unsafe {
            gl::BindVertexArray(self.vertex_array);
            gl::BindBuffer(gl::ELEMENT_ARRAY_BUFFER, self.index_buffer);
            gl::BufferData(
                gl::ELEMENT_ARRAY_BUFFER,
                (indices_len * core::mem::size_of::<u16>()) as GLsizeiptr,
                //mem::transmute(&indices.as_ptr()),
                indices.as_ptr() as *const gl::types::GLvoid,
                gl::STREAM_DRAW,
            );
        }

        let mut positions: Vec<f32> = Vec::with_capacity(2 * vertices_len);
        let mut tex_coords: Vec<f32> = Vec::with_capacity(2 * vertices_len);
        let mut colors: Vec<u8> = Vec::with_capacity(4 * vertices_len);
        for v in &mesh.vertices {
            positions.push(v.pos.x);
            positions.push(v.pos.y);

            tex_coords.push(v.uv.x);
            tex_coords.push(v.uv.y);

            colors.push(v.color[0]);
            colors.push(v.color[1]);
            colors.push(v.color[2]);
            colors.push(v.color[3]);
        }

        unsafe {
            gl::BindBuffer(gl::ARRAY_BUFFER, self.pos_buffer);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (positions.len() * core::mem::size_of::<f32>()) as GLsizeiptr,
                //mem::transmute(&positions.as_ptr()),
                positions.as_ptr() as *const gl::types::GLvoid,
                gl::STREAM_DRAW,
            );
        }

        let a_pos = CString::new("a_pos").unwrap();
        let a_pos_ptr = a_pos.as_ptr();
        let a_pos_loc = unsafe { gl::GetAttribLocation(self.program, a_pos_ptr) };
        assert!(a_pos_loc >= 0);
        let a_pos_loc = a_pos_loc as u32;

        let stride = 0;
        unsafe {
            gl::VertexAttribPointer(
                a_pos_loc,
                2,
                gl::FLOAT,
                gl::FALSE,
                stride,
                core::ptr::null(),
            );
            gl::EnableVertexAttribArray(a_pos_loc);

            gl::BindBuffer(gl::ARRAY_BUFFER, self.tc_buffer);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (tex_coords.len() * core::mem::size_of::<f32>()) as GLsizeiptr,
                //mem::transmute(&tex_coords.as_ptr()),
                tex_coords.as_ptr() as *const gl::types::GLvoid,
                gl::STREAM_DRAW,
            );
        }

        let a_tc = CString::new("a_tc").unwrap();
        let a_tc_ptr = a_tc.as_ptr();
        let a_tc_loc = unsafe { gl::GetAttribLocation(self.program, a_tc_ptr) };
        assert!(a_tc_loc >= 0);
        let a_tc_loc = a_tc_loc as u32;

        let stride = 0;
        unsafe {
            gl::VertexAttribPointer(a_tc_loc, 2, gl::FLOAT, gl::FALSE, stride, core::ptr::null());
            gl::EnableVertexAttribArray(a_tc_loc);

            gl::BindBuffer(gl::ARRAY_BUFFER, self.color_buffer);
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (colors.len() * core::mem::size_of::<u8>()) as GLsizeiptr,
                //mem::transmute(&colors.as_ptr()),
                colors.as_ptr() as *const gl::types::GLvoid,
                gl::STREAM_DRAW,
            );
        }

        let a_srgba = CString::new("a_srgba").unwrap();
        let a_srgba_ptr = a_srgba.as_ptr();
        let a_srgba_loc = unsafe { gl::GetAttribLocation(self.program, a_srgba_ptr) };
        assert!(a_srgba_loc >= 0);
        let a_srgba_loc = a_srgba_loc as u32;

        let stride = 0;
        unsafe {
            gl::VertexAttribPointer(
                a_srgba_loc,
                4,
                gl::UNSIGNED_BYTE,
                gl::FALSE,
                stride,
                core::ptr::null(),
            );
            gl::EnableVertexAttribArray(a_srgba_loc);

            gl::DrawElements(
                gl::TRIANGLES,
                indices_len as i32,
                gl::UNSIGNED_SHORT,
                core::ptr::null(),
            );
            gl::DisableVertexAttribArray(a_pos_loc);
            gl::DisableVertexAttribArray(a_tc_loc);
            gl::DisableVertexAttribArray(a_srgba_loc);
        }
    }

    pub fn set_texture(&mut self, tex_id: egui::TextureId, delta: &egui::epaint::ImageDelta) {
        let [w, h] = delta.image.size();

        if let Some([x, y]) = delta.pos {
            if let Some(texture) = self.textures.get_mut(&tex_id) {
                match &delta.image {
                    egui::ImageData::Color(image) => {
                        assert_eq!(
                            image.width() * image.height(),
                            image.pixels.len(),
                            "Mismatch between texture size and texel count"
                        );

                        let data: Vec<u8> =
                            image.pixels.iter().flat_map(|a| a.to_array()).collect();

                        texture.update_texture_part(x as _, y as _, w as _, h as _, &data);
                    }

                    egui::ImageData::Font(image) => {
                        assert_eq!(
                            image.width() * image.height(),
                            image.pixels.len(),
                            "Mismatch between texture size and texel count"
                        );

                        let gamma = 1.0;
                        let data: Vec<u8> = image
                            .srgba_pixels(gamma)
                            .flat_map(|a| a.to_array())
                            .collect();

                        texture.update_texture_part(x as _, y as _, w as _, h as _, &data);
                    }
                }
            } else {
                eprintln!("Failed to find egui texture {:?}", tex_id);
            }
        } else {
            let texture = match &delta.image {
                egui::ImageData::Color(image) => {
                    assert_eq!(
                        image.width() * image.height(),
                        image.pixels.len(),
                        "Mismatch between texture size and texel count"
                    );

                    let pixels: Vec<u8> = image.pixels.iter().flat_map(|a| a.to_array()).collect();

                    UserTexture {
                        size: (w, h),
                        pixels,
                        texture: None,
                        filtering: TextureFilter::Linear,
                        dirty: true,
                    }
                }
                egui::ImageData::Font(image) => {
                    assert_eq!(
                        image.width() * image.height(),
                        image.pixels.len(),
                        "Mismatch between texture size and texel count"
                    );

                    let gamma = 1.0;
                    let pixels: Vec<u8> = image
                        .srgba_pixels(gamma)
                        .flat_map(|a| a.to_array())
                        .collect();

                    UserTexture {
                        size: (w, h),
                        pixels,
                        texture: None,
                        filtering: TextureFilter::Linear,
                        dirty: true,
                    }
                }
            };

            let previous = self.textures.insert(tex_id, texture);
            if let Some(previous) = previous {
                previous.delete();
            }
        }
    }

    fn upload_user_textures(&mut self) {
        self.textures
            .values_mut()
            .filter(|user_texture| user_texture.texture.is_none() || user_texture.dirty)
            .for_each(|user_texture| {
                let pixels = std::mem::take(&mut user_texture.pixels);

                match user_texture.texture {
                    Some(texture) => unsafe {
                        gl::BindTexture(gl::TEXTURE_2D, texture);
                    },

                    None => {
                        let mut gl_texture = 0;
                        unsafe {
                            gl::GenTextures(1, &mut gl_texture);
                            gl::BindTexture(gl::TEXTURE_2D, gl_texture);
                            gl::TexParameteri(
                                gl::TEXTURE_2D,
                                gl::TEXTURE_WRAP_S,
                                gl::CLAMP_TO_EDGE as i32,
                            );
                            gl::TexParameteri(
                                gl::TEXTURE_2D,
                                gl::TEXTURE_WRAP_T,
                                gl::CLAMP_TO_EDGE as i32,
                            );
                        }

                        match user_texture.filtering {
                            TextureFilter::Nearest => unsafe {
                                gl::TexParameteri(
                                    gl::TEXTURE_2D,
                                    gl::TEXTURE_MIN_FILTER,
                                    gl::LINEAR as i32,
                                );
                                gl::TexParameteri(
                                    gl::TEXTURE_2D,
                                    gl::TEXTURE_MAG_FILTER,
                                    gl::LINEAR as i32,
                                );
                            },

                            TextureFilter::Linear => unsafe {
                                gl::TexParameteri(
                                    gl::TEXTURE_2D,
                                    gl::TEXTURE_MIN_FILTER,
                                    gl::NEAREST as i32,
                                );
                                gl::TexParameteri(
                                    gl::TEXTURE_2D,
                                    gl::TEXTURE_MAG_FILTER,
                                    gl::NEAREST as i32,
                                );
                            },
                        }
                        user_texture.texture = Some(gl_texture);
                    }
                }

                let level = 0;
                let internal_format = gl::RGBA;
                let border = 0;
                let src_format = gl::RGBA;
                let src_type = gl::UNSIGNED_BYTE;
                unsafe {
                    gl::TexImage2D(
                        gl::TEXTURE_2D,
                        level,
                        internal_format as i32,
                        user_texture.size.0 as i32,
                        user_texture.size.1 as i32,
                        border,
                        src_format,
                        src_type,
                        pixels.as_ptr() as *const c_void,
                    );
                }

                user_texture.dirty = false;
            });
    }

    pub fn free_texture(&mut self, tex_id: egui::TextureId) {
        if let Some(old_tex) = self.textures.remove(&tex_id) {
            old_tex.delete();
        }
    }
}
