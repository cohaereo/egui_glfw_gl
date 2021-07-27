#![warn(clippy::all)]
#![allow(clippy::single_match)]

// Re-export dependencies.
pub use egui;
pub use gl;
pub use glfw;

mod painter;

pub use painter::Painter;

use egui::*;

#[cfg(not(feature = "clipboard"))]
mod clipboard;

use clipboard::{
    ClipboardContext, // TODO: remove
    ClipboardProvider,
};

pub struct EguiInputState {
    pub pointer_pos: Pos2,
    pub clipboard: Option<ClipboardContext>,
    pub input: RawInput,
    pub modifiers: Modifiers,
}

impl EguiInputState {
    pub fn new(input: RawInput) -> Self {
        EguiInputState {
            pointer_pos: Pos2::new(0f32, 0f32),
            clipboard: init_clipboard(),
            input,
            modifiers: Modifiers::default(),
        }
    }
}

pub fn handle_event(event: glfw::WindowEvent, state: &mut EguiInputState) {
    use glfw::WindowEvent::*;

    match event {
        FramebufferSize(width, height) => {
            state.input.screen_rect = Some(Rect::from_min_size(
                Pos2::new(0f32, 0f32),
                egui::vec2(width as f32, height as f32) / state.input.pixels_per_point.unwrap(),
            ))
        }

        MouseButton (mouse_btn, glfw::Action::Press, _) => state.input.events.push(egui::Event::PointerButton {
            pos: state.pointer_pos,
            button: match mouse_btn {
                glfw::MouseButtonLeft => egui::PointerButton::Primary,
                glfw::MouseButtonRight => egui::PointerButton::Secondary,
                glfw::MouseButtonMiddle => egui::PointerButton::Middle,
                _ => unreachable!(),
            },
            pressed: true,
            modifiers: state.modifiers,
        }),

        MouseButton (mouse_btn, glfw::Action::Release, _) => state.input.events.push(egui::Event::PointerButton {
            pos: state.pointer_pos,
            button: match mouse_btn {
                glfw::MouseButtonLeft => egui::PointerButton::Primary,
                glfw::MouseButtonRight => egui::PointerButton::Secondary,
                glfw::MouseButtonMiddle => egui::PointerButton::Middle,
                _ => unreachable!(),
            },
            pressed: false,
            modifiers: state.modifiers,
        }),

        CursorPos(x, y) => {
            state.pointer_pos = pos2(
                x as f32 / state.input.pixels_per_point.unwrap(),
                y as f32 / state.input.pixels_per_point.unwrap(),
            );
            state
                .input
                .events
                .push(egui::Event::PointerMoved(state.pointer_pos))
        }

        Key(keycode, _scancode, glfw::Action::Release, keymod) => {
            use glfw::Modifiers as Mod;
            if let Some(key) = translate_virtual_key_code(keycode) {
                state.modifiers = Modifiers {
                    alt: (keymod & Mod::Alt == Mod::Alt),
                    ctrl: (keymod & Mod::Control == Mod::Control),
                    shift: (keymod & Mod::Shift == Mod::Shift),

                    // TODO: GLFW doesn't seem to support the mac command key
                    // mac_cmd: keymod & Mod::LGUIMOD == Mod::LGUIMOD,
                    command: (keymod & Mod::Control == Mod::Control),

                    ..Default::default()
                };

                state.input.events.push(Event::Key {
                    key,
                    pressed: false,
                    modifiers: state.modifiers,
                });
            }
        }

        Key(keycode, _scancode, glfw::Action::Press | glfw::Action::Repeat, keymod) => {
            use glfw::Modifiers as Mod;
            if let Some(key) = translate_virtual_key_code(keycode) {
                state.modifiers = Modifiers {
                    alt: (keymod & Mod::Alt == Mod::Alt),
                    ctrl: (keymod & Mod::Control == Mod::Control),
                    shift: (keymod & Mod::Shift == Mod::Shift),

                    // TODO: GLFW doesn't seem to support the mac command key
                    // mac_cmd: keymod & Mod::LGUIMOD == Mod::LGUIMOD,
                    command: (keymod & Mod::Control == Mod::Control),

                    ..Default::default()
                };

                if state.modifiers.command && key == egui::Key::X {
                    state.input.events.push(egui::Event::Cut);
                } else if state.modifiers.command && key == egui::Key::C {
                    state.input.events.push(egui::Event::Copy);
                } else if state.modifiers.command && key == egui::Key::V {
                    if let Some(clipboard_ctx) = state.clipboard.as_mut() {
                        state.input.events.push(egui::Event::Text(clipboard_ctx.get_contents().unwrap_or("".to_string())));
                    }
                } else {
                    state.input.events.push(Event::Key {
                        key,
                        pressed: true,
                        modifiers: state.modifiers,
                    });
                }
            }
        }

        Char(c) => {
            state.input.events.push(Event::Text(c.to_string()));
        }

        Scroll (x, y) => {
            state.input.scroll_delta = vec2(x as f32, y as f32);
        }

        _ => {}
    }
}

pub fn translate_virtual_key_code(key: glfw::Key) -> Option<egui::Key> {
    use glfw::Key::*;

    Some(match key {
        Left => Key::ArrowLeft,
        Up => Key::ArrowUp,
        Right => Key::ArrowRight,
        Down => Key::ArrowDown,

        Escape => Key::Escape,
        Tab => Key::Tab,
        Backspace => Key::Backspace,
        Space => Key::Space,

        Enter => Key::Enter,

        Insert => Key::Insert,
        Home => Key::Home,
        Delete => Key::Delete,
        End => Key::End,
        PageDown => Key::PageDown,
        PageUp => Key::PageUp,


        A => Key::A,
        B => Key::B,
        C => Key::C,
        D => Key::D,
        E => Key::E,
        F => Key::F,
        G => Key::G,
        H => Key::H,
        I => Key::I,
        J => Key::J,
        K => Key::K,
        L => Key::L,
        M => Key::M,
        N => Key::N,
        O => Key::O,
        P => Key::P,
        Q => Key::Q,
        R => Key::R,
        S => Key::S,
        T => Key::T,
        U => Key::U,
        V => Key::V,
        W => Key::W,
        X => Key::X,
        Y => Key::Y,
        Z => Key::Z,

        _ => {
            return None;
        }
    })
}

pub fn translate_cursor(cursor_icon: egui::CursorIcon) -> glfw::StandardCursor {
    match cursor_icon {
        CursorIcon::Default => glfw::StandardCursor::Arrow,
        CursorIcon::PointingHand => glfw::StandardCursor::Hand,
        CursorIcon::ResizeHorizontal => glfw::StandardCursor::HResize,
        CursorIcon::ResizeVertical => glfw::StandardCursor::VResize,
        // TODO: GLFW doesnt have these specific resize cursors, so we'll just use the HResize and VResize ones instead
        CursorIcon::ResizeNeSw => glfw::StandardCursor::HResize,
        CursorIcon::ResizeNwSe => glfw::StandardCursor::VResize,
        CursorIcon::Text => glfw::StandardCursor::IBeam,
        CursorIcon::Crosshair => glfw::StandardCursor::Crosshair,
        // TODO: Same for these
        CursorIcon::NotAllowed | CursorIcon::NoDrop => glfw::StandardCursor::Arrow,
        CursorIcon::Wait => glfw::StandardCursor::Arrow,
        CursorIcon::Grab | CursorIcon::Grabbing => glfw::StandardCursor::Hand,

        _ => glfw::StandardCursor::Arrow,
    }
}

pub fn init_clipboard() -> Option<ClipboardContext> {
    match ClipboardContext::new() {
        Ok(clipboard) => Some(clipboard),
        Err(err) => {
            eprintln!("Failed to initialize clipboard: {}", err);
            None
        }
    }
}

pub fn copy_to_clipboard(egui_state: &mut EguiInputState, copy_text: String) {
    if let Some(clipboard) = egui_state.clipboard.as_mut() {
        let result = clipboard.set_contents(copy_text);
        if result.is_err() {
            dbg!("Unable to set clipboard content.");
        }
    }
}
