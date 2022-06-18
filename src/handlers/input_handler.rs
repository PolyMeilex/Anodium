use crate::{data::seat::SeatState, CalloopData, State};
use anodium_backend::{InputHandler, OutputId};

use smithay::{
    backend::input::{
        Event, InputEvent, KeyState, KeyboardKeyEvent, PointerButtonEvent,
        PointerMotionAbsoluteEvent, PointerMotionEvent,
    },
    desktop::WindowSurfaceType,
    reexports::wayland_server::protocol::wl_pointer,
    utils::{Logical, Point},
    wayland::{
        seat::{keysyms as xkb, ButtonEvent, FilterResult, MotionEvent, PointerHandle},
        SERIAL_COUNTER,
    },
};

impl InputHandler for CalloopData {
    fn process_input_event<I: smithay::backend::input::InputBackend>(
        &mut self,
        event: InputEvent<I>,
        output_id: Option<&OutputId>,
    ) {
        let absolute_output = self
            .state
            .space
            .outputs()
            .find(|o| o.user_data().get::<OutputId>() == output_id)
            .cloned();

        match event {
            InputEvent::Keyboard { event } => {
                let keyboard = self.state.seat.get_keyboard().unwrap();

                let state = event.state();

                keyboard.input::<(), _>(
                    &self.display.handle(),
                    event.key_code(),
                    event.state(),
                    SERIAL_COUNTER.next_serial(),
                    event.time(),
                    |_modifiers, handle| {
                        let keysym = handle.modified_sym();

                        if keysym == xkb::KEY_Escape {
                            self.state.loop_signal.stop();
                        }

                        if keysym == xkb::KEY_t && event.state() == KeyState::Released {
                            std::process::Command::new("gtk4-demo").spawn().ok();
                        }

                        SeatState::for_seat(&self.state.seat).update_pressed_keys(keysym, state);

                        // self.config.key_action(modifiers, &handle, state);

                        FilterResult::Forward
                    },
                );
            }
            InputEvent::PointerMotion { event } => {
                let pointer = self.state.seat.get_pointer().unwrap();
                let seat_state = SeatState::for_seat(&self.state.seat);

                let mut position = seat_state.pointer_pos() + event.delta();

                let max_x = self.state.space.outputs().fold(0, |acc, o| {
                    acc + self.state.space.output_geometry(o).unwrap().size.w
                });

                let max_y = self
                    .state
                    .space
                    .outputs()
                    .next()
                    .map(|o| self.state.space.output_geometry(o).unwrap().size.h)
                    .unwrap_or_default();

                position.x = position.x.max(0.0).min(max_x as f64 - 1.0);
                position.y = position.y.max(0.0).min(max_y as f64 - 1.0);

                seat_state.set_pointer_pos(position);
                self.state.pointer_motion(pointer, position, event.time());
            }
            InputEvent::PointerMotionAbsolute { event } => {
                let pointer = self.state.seat.get_pointer().unwrap();

                let output = absolute_output
                    .unwrap_or_else(|| self.state.space.outputs().next().unwrap().clone());
                let output_geo = self.state.space.output_geometry(&output).unwrap();
                let output_loc = output_geo.loc.to_f64();

                let position = output_loc + event.position_transformed(output_geo.size);

                SeatState::for_seat(&self.state.seat).set_pointer_pos(position);
                self.state.pointer_motion(pointer, position, event.time());
            }
            InputEvent::PointerButton { event } => {
                let dh = &self.display.handle();

                let pointer = self.state.seat.get_pointer().unwrap();
                let keyboard = self.state.seat.get_keyboard().unwrap();

                let pointer_pos = SeatState::for_seat(&self.state.seat).pointer_pos();

                let serial = SERIAL_COUNTER.next_serial();

                let button = event.button_code();

                let button_state = wl_pointer::ButtonState::from(event.state());

                if wl_pointer::ButtonState::Pressed == button_state && !pointer.is_grabbed() {
                    if let Some(window) = self.state.space.window_under(pointer_pos).cloned() {
                        self.state.space.raise_window(&window, true);
                        keyboard.set_focus(dh, Some(window.toplevel().wl_surface()), serial);
                        window.set_activated(true);
                        window.configure();
                    } else {
                        self.state.space.windows().for_each(|window| {
                            window.set_activated(false);
                            window.configure();
                        });
                        keyboard.set_focus(dh, None, serial);
                    }
                };

                pointer.button(
                    &mut self.state,
                    dh,
                    &ButtonEvent {
                        button,
                        state: button_state,
                        serial,
                        time: event.time(),
                    },
                );
            }
            InputEvent::PointerAxis { event } => {
                let frame = anodium_framework::input::basic_axis_frame::<I>(&event);

                let pointer = self.state.seat.get_pointer().unwrap();
                pointer.axis(&mut self.state, &self.display.handle(), frame);
            }
            _ => {}
        }
    }
}

impl State {
    fn pointer_motion(
        &mut self,
        pointer: PointerHandle<Self>,
        position: Point<f64, Logical>,
        time: u32,
    ) {
        let under = self
            .space
            .surface_under(position, WindowSurfaceType::all())
            .map(|(_, surface, location)| (surface, location));

        let dh = self.display.clone();
        pointer.motion(
            self,
            &dh,
            &MotionEvent {
                location: position,
                focus: under,
                serial: SERIAL_COUNTER.next_serial(),
                time,
            },
        );
    }
}
