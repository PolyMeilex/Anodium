use imgui::Io;

use smithay::backend::input::{
    Axis, AxisSource, ButtonState, InputBackend, InputEvent, KeyState, KeyboardKeyEvent,
    MouseButton, PointerAxisEvent, PointerButtonEvent, PointerMotionAbsoluteEvent,
};

pub fn handle_event<I: InputBackend>(io: &mut Io, evt: InputEvent<I>) {
    match evt {
        InputEvent::PointerMotionAbsolute { event, .. } => {
            let position = event.position();
            io.mouse_pos = [position.x as f32, position.y as f32];
        }

        InputEvent::Keyboard { event, .. } => {
            let keycode = event.key_code();
            let state = event.state();
            match state {
                KeyState::Pressed => io.keys_down[keycode as usize] = true,
                KeyState::Released => io.keys_down[keycode as usize] = false,
            }
        }

        InputEvent::PointerButton { event, .. } => {
            let button = event.button().unwrap();
            let state = event.state() == ButtonState::Pressed;

            match button {
                MouseButton::Left => io.mouse_down[0] = state,
                MouseButton::Right => io.mouse_down[1] = state,
                MouseButton::Middle => io.mouse_down[2] = state,
                _ => {}
            };
        }

        InputEvent::PointerAxis { event, .. } => match event.source() {
            AxisSource::Wheel => {
                let amount_discrete = event.amount_discrete(Axis::Vertical).unwrap();
                io.mouse_wheel += amount_discrete as f32;
            }
            _ => {}
        },
        _ => {}
    }
}
