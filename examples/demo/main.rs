//Alias the backend to something less mouthful
use egui_glfw_gl as egui_backend;

use std::time::Instant;

use egui_backend::egui::{vec2, Color32, Image, Pos2, Rect};
use egui_glfw_gl::glfw::{Context, Key};
use glfw::Action;

const SCREEN_WIDTH: u32 = 800;
const SCREEN_HEIGHT: u32 = 600;
const PIC_WIDTH: i32 = 320;
const PIC_HEIGHT: i32 = 192;
mod triangle;

fn main() {
    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS).unwrap();
    glfw.window_hint(glfw::WindowHint::ContextVersion(3, 2));
    glfw.window_hint(glfw::WindowHint::OpenGlProfile(glfw::OpenGlProfileHint::Core));
    glfw.window_hint(glfw::WindowHint::DoubleBuffer(true));
    glfw.window_hint(glfw::WindowHint::Resizable(false));

    let (mut window, events) = glfw.create_window(SCREEN_WIDTH, SCREEN_HEIGHT, "Egui in GLFW!", glfw::WindowMode::Windowed)
        .expect("Failed to create GLFW window.");

    window.set_char_polling(true);
    window.set_cursor_pos_polling(true);
    window.set_key_polling(true);
    window.set_mouse_button_polling(true);
    window.make_current();
    glfw.set_swap_interval(glfw::SwapInterval::Sync(1));

    // let dpi: f32 = glfw.with_primary_monitor(|_, monitor| {
    //     let phys_size = monitor.unwrap().get_physical_size();
    //     let screen_width = monitor.unwrap().get_video_mode().unwrap().width;
    //
    //
    //     screen_width as f32 / ((phys_size.0 as f32 / 10.) / 2.54)
    // });

    // FIXME: Can't get scaling right just yet, using 1 pixel per point *for now*
    let native_pixels_per_point = 1.;
    // let native_pixels_per_point = 96. / dpi;


    gl::load_with(|symbol| window.get_proc_address(symbol) as *const _);

    let mut painter = egui_backend::Painter::new(&mut window, SCREEN_WIDTH, SCREEN_HEIGHT);
    let mut egui_ctx = egui::CtxRef::default();

    let (width, height) = window.get_framebuffer_size();

    let mut egui_input_state = egui_backend::EguiInputState::new(egui::RawInput {
        screen_rect: Some(Rect::from_min_size(
            Pos2::new(0f32, 0f32),
            vec2(width as f32, height as f32) / native_pixels_per_point,
        )),
        pixels_per_point: Some(native_pixels_per_point),
        ..Default::default()
    });
    let start_time = Instant::now();
    let mut srgba: Vec<Color32> = Vec::new();

    //For now we will just set everything to black, because
    //we will be updating it dynamically later. However, this could just as
    //easily have been some actual picture data loaded in.
    for _ in 0..PIC_HEIGHT {
        for _ in 0..PIC_WIDTH {
            srgba.push(Color32::BLACK);
        }
    }

    //The user texture is what allows us to mix Egui and GL rendering contexts.
    //Egui just needs the texture id, as the actual texture is managed by the backend.
    let plot_tex_id =
        painter.new_user_texture((PIC_WIDTH as usize, PIC_HEIGHT as usize), &srgba, false);

    //Some variables to help draw a sine wave
    let mut sine_shift = 0f32;

    let mut amplitude: f32 = 50f32;
    let mut test_str: String =
        "A text box to write in. Cut, copy, paste commands are available.".to_owned();

    //We will draw a crisp white triangle using OpenGL.
    let triangle = triangle::Triangle::new();
    let mut quit = false;

    while !window.should_close() {
        egui_input_state.input.time = Some(start_time.elapsed().as_secs_f64());
        egui_ctx.begin_frame(egui_input_state.input.take());

        //In egui 0.10.0 we seem to be losing the value to pixels_per_point,
        //so setting it every frame now.
        //TODO: Investigate if this is the right way.
        egui_input_state.input.pixels_per_point = Some(native_pixels_per_point);

        //An example of how OpenGL can be used to draw custom stuff with egui
        //overlaying it:
        //First clear the background to something nice.
        unsafe {
            // Clear the screen to black
            gl::ClearColor(0.455, 0.302, 0.663, 1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
        //Then draw our triangle.
        triangle.draw();

        let mut srgba: Vec<Color32> = Vec::new();
        let mut angle = 0f32;
        //Draw a cool sine wave in a buffer.
        for y in 0..PIC_HEIGHT {
            for x in 0..PIC_WIDTH {
                srgba.push(Color32::BLACK);
                if y == PIC_HEIGHT - 1 {
                    let y = amplitude * (angle * 3.142f32 / 180f32 + sine_shift).sin();
                    let y = PIC_HEIGHT as f32 / 2f32 - y;
                    srgba[(y as i32 * PIC_WIDTH + x) as usize] = Color32::YELLOW;
                    angle += 360f32 / PIC_WIDTH as f32;
                }
            }
        }
        sine_shift += 0.1f32;

        //This updates the previously initialized texture with new data.
        //If we weren't updating the texture, this call wouldn't be required.
        painter.update_user_texture_data(plot_tex_id, &srgba);

        egui::Window::new("Egui with GLFW").show(&egui_ctx, |ui| {
            //Image just needs a texture id reference, so we just pass it the texture id that was returned to us
            //when we previously initialized the texture.
            ui.add(Image::new(plot_tex_id, vec2(PIC_WIDTH as f32, PIC_HEIGHT as f32)));
            ui.separator();
            ui.label("A simple sine wave plotted onto a GL texture then blitted to an egui managed Image.");
            ui.label(" ");
            ui.text_edit_multiline(&mut test_str);
            ui.label(" ");
            
            ui.add(egui::Slider::new(&mut amplitude, 0.0..=50.0).text("Amplitude"));
            ui.label(" ");
            if ui.button("Quit").clicked() {
                quit = true;
            }
        });

        let (egui_output, paint_cmds) = egui_ctx.end_frame();

        //Handle cut, copy text from egui
        if !egui_output.copied_text.is_empty() {
            egui_backend::copy_to_clipboard(&mut egui_input_state, egui_output.copied_text);
        }

        let paint_jobs = egui_ctx.tessellate(paint_cmds);

        //Note: passing a bg_color to paint_jobs will clear any previously drawn stuff.
        //Use this only if egui is being used for all drawing and you aren't mixing your own Open GL
        //drawing calls with it.
        //Since we are custom drawing an OpenGL Triangle we don't need egui to clear the background.
        painter.paint_jobs(
            None,
            paint_jobs,
            &egui_ctx.texture(),
            native_pixels_per_point,
        );

        for (_, event) in glfw::flush_messages(&events) {
            match event {
                glfw::WindowEvent::Close => window.set_should_close(true),
                _ => { egui_backend::handle_event(event, &mut egui_input_state); }
            }
        }
        window.swap_buffers();
        glfw.poll_events();

        if quit {
            break;
        }
    }
}
