use {
    crate::State,
    anodium_backend::InputHandler,
    smithay::{
        backend::input::{
            ButtonState, Event, InputEvent, KeyboardKeyEvent, PointerButtonEvent,
            PointerMotionAbsoluteEvent, PointerMotionEvent,
        },
        desktop::WindowSurfaceType,
        reexports::wayland_server::protocol::wl_pointer,
        wayland::{
            seat::{keysyms as xkb, FilterResult},
            SERIAL_COUNTER,
        },
    },
};

impl InputHandler for State {
    fn process_input_event<I: smithay::backend::input::InputBackend>(
        &mut self,
        event: InputEvent<I>,
        absolute_output: Option<&smithay::wayland::output::Output>,
    ) {
        match event {
            InputEvent::Keyboard { event } => {
                let keyboard = self.seat.get_keyboard().unwrap();

                keyboard.input::<(), _>(
                    event.key_code(),
                    event.state(),
                    SERIAL_COUNTER.next_serial(),
                    event.time(),
                    |_modifiers, handle| {
                        assert!(handle.modified_sym() != xkb::KEY_Escape);

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

                let mut position = pointer.current_location();
                position += event.delta();

                let under = self.space.window_under(position).and_then(|win| {
                    let window_loc = self.space.window_location(win).unwrap();
                    win.surface_under(position - window_loc.to_f64(), WindowSurfaceType::all())
                        .map(|(s, loc)| (s, loc + window_loc))
                });

                pointer.motion(
                    position,
                    under,
                    SERIAL_COUNTER.next_serial(),
                    event.time(),
                    self,
                );
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let pointer = self.seat.get_pointer().unwrap();

                let output =
                    absolute_output.unwrap_or_else(|| self.space.outputs().next().unwrap());
                let output_geo = self.space.output_geometry(output).unwrap();
                let output_loc = output_geo.loc.to_f64();

                let position = output_loc + event.position_transformed(output_geo.size);

                let under = self.space.window_under(position).and_then(|win| {
                    let window_loc = self.space.window_location(win).unwrap();
                    win.surface_under(position - window_loc.to_f64(), WindowSurfaceType::all())
                        .map(|(s, loc)| (s, loc + window_loc))
                });

                pointer.motion(
                    position,
                    under,
                    SERIAL_COUNTER.next_serial(),
                    event.time(),
                    self,
                );
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
            _ => {}
        }
    }
}
