use crate::State;
use anodium_backend::{InputHandler, OutputId};

use smithay::{
    backend::input::{
        ButtonState, Event, InputEvent, KeyboardKeyEvent, PointerButtonEvent,
        PointerMotionAbsoluteEvent, PointerMotionEvent,
    },
    desktop::WindowSurfaceType,
    reexports::wayland_server::protocol::wl_pointer,
    utils::{Logical, Point},
    wayland::{
        seat::{keysyms as xkb, FilterResult, PointerHandle},
        SERIAL_COUNTER,
    },
};

impl InputHandler for State {
    fn process_input_event<I: smithay::backend::input::InputBackend>(
        &mut self,
        event: InputEvent<I>,
        output_id: Option<&OutputId>,
    ) {
        let absolute_output = self
            .space
            .outputs()
            .find(|o| o.user_data().get::<OutputId>() == output_id)
            .cloned();

        match event {
            InputEvent::Keyboard { event } => {
                let keyboard = self.seat.get_keyboard().unwrap();

                keyboard.input::<(), _>(
                    event.key_code(),
                    event.state(),
                    SERIAL_COUNTER.next_serial(),
                    event.time(),
                    |_modifiers, handle| {
                        if handle.modified_sym() == xkb::KEY_Escape {
                            self.loop_signal.stop();
                        }

                        if handle.modified_sym() == xkb::KEY_C {
                            std::process::Command::new("nautilus")
                                .env("WAYLAND_DISPLAY", "wayland-1")
                                .spawn()
                                .unwrap();
                        }

                        FilterResult::Forward
                    },
                );
            }
            InputEvent::PointerMotion { event } => {
                let pointer = self.seat.get_pointer().unwrap();

                let mut position = pointer.current_location() + event.delta();

                let max_x = self.space.outputs().fold(0, |acc, o| {
                    acc + self.space.output_geometry(o).unwrap().size.w
                });

                let max_y = self
                    .space
                    .outputs()
                    .next()
                    .map(|o| self.space.output_geometry(o).unwrap().size.h)
                    .unwrap_or_default();

                position.x = position.x.max(0.0).min(max_x as f64 - 1.0);
                position.y = position.y.max(0.0).min(max_y as f64 - 1.0);

                self.pointer_motion(pointer, position, event.time());
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let pointer = self.seat.get_pointer().unwrap();

                let output =
                    absolute_output.unwrap_or_else(|| self.space.outputs().next().unwrap().clone());
                let output_geo = self.space.output_geometry(&output).unwrap();
                let output_loc = output_geo.loc.to_f64();

                let position = output_loc + event.position_transformed(output_geo.size);

                self.pointer_motion(pointer, position, event.time());
            }
            InputEvent::PointerButton { event } => {
                let pointer = self.seat.get_pointer().unwrap();
                let keyboard = self.seat.get_keyboard().unwrap();

                let serial = SERIAL_COUNTER.next_serial();
                let button = event.button_code();
                let state = match event.state() {
                    ButtonState::Pressed => wl_pointer::ButtonState::Pressed,
                    ButtonState::Released => wl_pointer::ButtonState::Released,
                };

                pointer.button(button, state, serial, event.time(), self);

                let position = pointer.current_location();
                let under = self
                    .space
                    .window_under(position)
                    .and_then(|win| {
                        let pos = self.space.window_location(win).unwrap().to_f64();
                        win.surface_under(position - pos, WindowSurfaceType::all())
                    })
                    .map(|w| w.0);

                keyboard.set_focus(under.as_ref(), serial)
            }
            InputEvent::PointerAxis { event } => {
                let frame = anodium_framework::input::basic_axis_frame::<I>(&event);

                let pointer = self.seat.get_pointer().unwrap();
                pointer.axis(frame, self);
            }
            _ => {}
        }
    }
}

impl State {
    fn pointer_motion(&mut self, pointer: PointerHandle, position: Point<f64, Logical>, time: u32) {
        let under = self.space.window_under(position).and_then(|win| {
            let window_loc = self.space.window_location(win).unwrap();
            win.surface_under(position - window_loc.to_f64(), WindowSurfaceType::all())
                .map(|(s, loc)| (s, loc + window_loc))
        });

        pointer.motion(position, under, SERIAL_COUNTER.next_serial(), time, self);
    }
}
