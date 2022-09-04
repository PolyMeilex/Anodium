use anodium_backend::{InputHandler, OutputId};
use smithay::{
    backend::input::{
        AbsolutePositionEvent, ButtonState, Event, InputEvent, KeyState, KeyboardKeyEvent,
        PointerButtonEvent, PointerMotionEvent,
    },
    desktop::{self, WindowSurfaceType},
    input::{
        keyboard::{keysyms as xkb, FilterResult},
        pointer::{ButtonEvent, Focus, GrabStartData, MotionEvent, PointerHandle},
    },
    utils::{Logical, Point, SERIAL_COUNTER},
};

use crate::{data::seat::SeatState, grabs::MoveSurfaceGrab, CalloopData, State};

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

                let key_state = event.state();

                keyboard.input::<(), _>(
                    &mut self.state,
                    event.key_code(),
                    event.state(),
                    SERIAL_COUNTER.next_serial(),
                    event.time(),
                    |state, modifiers, handle| {
                        let keysym = handle.modified_sym();

                        SeatState::for_seat(&state.seat).update_pressed_keys(keysym, key_state);

                        if keysym == xkb::KEY_Escape {
                            state.loop_signal.stop();
                        }

                        if keysym == xkb::KEY_t
                            && modifiers.alt
                            && event.state() == KeyState::Pressed
                        {
                            std::process::Command::new("weston-terminal").spawn().ok();

                            FilterResult::Intercept(())
                        } else if keysym == xkb::KEY_g
                            && modifiers.alt
                            && event.state() == KeyState::Pressed
                        {
                            std::process::Command::new("gtk4-demo").spawn().ok();

                            FilterResult::Intercept(())
                        } else {
                            FilterResult::Forward
                        }
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
                let pointer = self.state.seat.get_pointer().unwrap();
                let keyboard = self.state.seat.get_keyboard().unwrap();

                let serial = SERIAL_COUNTER.next_serial();

                let button = event.button_code();
                let button_state = event.state();

                let seat_state = SeatState::for_seat(&self.state.seat);
                let pointer_pos = seat_state.pointer_pos();
                let is_alt_pressed = seat_state.is_key_pressed(xkb::KEY_Alt_L);

                if ButtonState::Pressed == button_state {
                    let window_under = self.state.space.window_under(pointer_pos).cloned();

                    if !pointer.is_grabbed() {
                        if let Some(window) = window_under {
                            activate_and_brind_to_top(&mut self.state.space, &window);

                            keyboard.set_focus(
                                &mut self.state,
                                Some(window.toplevel().wl_surface().clone()),
                                serial,
                            );

                            // Check for compositor initiated move grab
                            if is_alt_pressed {
                                let start_data = GrabStartData {
                                    focus: None,
                                    button,
                                    location: pointer_pos,
                                };

                                let initial_window_location =
                                    self.state.space.window_location(&window).unwrap();

                                let grab = MoveSurfaceGrab {
                                    start_data,
                                    window,
                                    initial_window_location,
                                };

                                pointer.set_grab(&mut self.state, grab, serial, Focus::Clear);

                                // Return early, we don't want to send button event to this window/surface
                                return;
                            }
                        } else {
                            self.state.space.windows().for_each(|window| {
                                window.set_activated(false);

                                // TODO: Remove once smithay supports xwayland
                                if let desktop::Kind::Xdg(_) = window.toplevel() {
                                    window.configure();
                                }
                            });
                            keyboard.set_focus(&mut self.state, None, serial);
                        }
                    };
                }

                pointer.button(
                    &mut self.state,
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
                pointer.axis(&mut self.state, frame);
            }
            _ => {}
        }
    }
}

fn activate_and_brind_to_top(space: &mut desktop::Space, window: &desktop::Window) {
    space.windows().filter(|w| *w != window).for_each(|window| {
        window.set_activated(false);

        // TODO: Remove once smithay supports xwayland
        if let desktop::Kind::Xdg(_) = window.toplevel() {
            window.configure();
        }
    });

    space.raise_window(window, true);
    window.set_activated(true);

    // TODO: Remove once smithay supports xwayland
    if let desktop::Kind::Xdg(_) = window.toplevel() {
        window.configure();
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

        pointer.motion(
            self,
            under,
            &MotionEvent {
                location: position,
                serial: SERIAL_COUNTER.next_serial(),
                time,
            },
        );
    }
}
