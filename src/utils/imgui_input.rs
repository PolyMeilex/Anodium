use imgui::Io;

use smithay::{
    backend::input::{
        Axis, AxisSource, ButtonState, InputBackend, InputEvent, KeyState, KeyboardKeyEvent,
        MouseButton, PointerAxisEvent, PointerButtonEvent, PointerMotionAbsoluteEvent,
        PointerMotionEvent,
    },
    utils::{Logical, Point},
};

use crate::output_map::Output;

pub fn handle_event<I: InputBackend>(
    io: &mut Io,
    evt: InputEvent<I>,
    output: &Output,
    mouse_location: Point<f64, Logical>,
) {
    let output_location = output.location().to_f64();
    let absolute_mouse_location = mouse_location - output.location().to_f64();

    io.mouse_pos[0] = absolute_mouse_location.x as f32;
    io.mouse_pos[1] = absolute_mouse_location.y as f32;

    match evt {
        /*InputEvent::PointerMotion { event, .. } => {
            let delta = event.delta();
            io.mouse_pos[0] += delta.x as f32;
            io.mouse_pos[1] += delta.y as f32;
        }

        InputEvent::PointerMotionAbsolute { event, .. } => {
            let output_size = output.size();
            let position = event.position_transformed(output_size);
            io.mouse_pos = [position.x as f32, position.y as f32];
            info!("position: {:?}", position);
        }*/
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
