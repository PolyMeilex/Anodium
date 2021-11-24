use std::{
    process::{self, Command},
    sync::atomic::Ordering,
};

use crate::{config, grabs::MoveSurfaceGrab, Anodium};

use smithay::{
    backend::{
        input::{
            self, Event, InputBackend, InputEvent, KeyState, KeyboardKeyEvent, PointerAxisEvent,
            PointerButtonEvent, PointerMotionAbsoluteEvent, PointerMotionEvent,
        },
        session::Session,
    },
    reexports::wayland_server::protocol::wl_pointer,
    utils::{Logical, Point},
    wayland::{
        seat::{keysyms as xkb, AxisFrame, FilterResult, Keysym, ModifiersState},
        SERIAL_COUNTER as SCOUNTER,
    },
};

impl Anodium {
    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event, .. } => {
                let action = self.keyboard_key_to_action::<I>(event);
                self.shortcut_handler(action)
            }
            InputEvent::PointerMotion { event, .. } => {
                self.input_state.pointer_location =
                    self.clamp_coords(self.input_state.pointer_location + event.delta());
                self.on_pointer_move(event.time());
            }
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let output_size = self.output_map.find_by_index(0).map(|o| o.size());

                if let Some(output_size) = output_size {
                    self.input_state.pointer_location = event.position_transformed(output_size);
                    self.on_pointer_move(event.time());
                }
            }
            InputEvent::PointerButton { event, .. } => self.on_pointer_button::<I>(event),
            InputEvent::PointerAxis { event, .. } => self.on_pointer_axis::<I>(event),
            _ => {}
        }
    }

    fn keyboard_key_to_action<I: InputBackend>(&mut self, evt: I::KeyboardKeyEvent) -> KeyAction {
        let keycode = evt.key_code();
        let state = evt.state();
        debug!("key"; "keycode" => keycode, "state" => format!("{:?}", state));
        let serial = SCOUNTER.next_serial();
        let time = Event::time(&evt);

        let modifiers_state = &mut self.input_state.modifiers_state;
        let suppressed_keys = &mut self.input_state.suppressed_keys;
        let pressed_keys = &mut self.input_state.pressed_keys;
        let configvm = self.config.clone();

        self.input_state
            .keyboard
            .input(keycode, state, serial, time, |modifiers, handle| {
                let keysym = handle.modified_sym();

                if let KeyState::Pressed = state {
                    pressed_keys.insert(keysym);
                } else {
                    pressed_keys.remove(&keysym);
                }

                let keysym_desc = ::xkbcommon::xkb::keysym_get_name(keysym);

                debug!( "keysym";
                    "state" => format!("{:?}", state),
                    "mods" => format!("{:?}", modifiers),
                    "keysym" => &keysym_desc
                );
                *modifiers_state = *modifiers;

                // If the key is pressed and triggered a action
                // we will not forward the key to the client.
                // Additionally add the key to the suppressed keys
                // so that we can decide on a release if the key
                // should be forwarded to the client or not.

                if let KeyState::Pressed = state {
                    let action = process_keyboard_shortcut(*modifiers, keysym);

                    if action.is_some() {
                        suppressed_keys.push(keysym);
                    } else {
                        if config::keyboard::key_action(
                            &configvm,
                            &keysym_desc,
                            state,
                            pressed_keys,
                        ) {
                            suppressed_keys.push(keysym);
                            return FilterResult::Intercept(KeyAction::None);
                        }
                    }

                    action
                        .map(FilterResult::Intercept)
                        .unwrap_or(FilterResult::Forward)
                } else {
                    let suppressed = suppressed_keys.contains(&keysym);
                    if suppressed {
                        suppressed_keys.retain(|k| *k != keysym);
                        FilterResult::Intercept(KeyAction::None)
                    } else {
                        FilterResult::Forward
                    }
                }
            })
            .unwrap_or(KeyAction::None)
    }

    fn on_pointer_button<I: InputBackend>(&mut self, evt: I::PointerButtonEvent) {
        let serial = SCOUNTER.next_serial();

        debug!("Mouse Event"; "Mouse button" => format!("{:?}", evt.button()));

        let button = evt.button_code();
        let state = match evt.state() {
            input::ButtonState::Pressed => {
                // change the keyboard focus unless the pointer is grabbed
                if !self.input_state.pointer.is_grabbed() {
                    let under = self.surface_under(self.input_state.pointer_location);

                    self.input_state
                        .keyboard
                        .set_focus(under.as_ref().map(|&(ref s, _)| s), serial);
                }
                wl_pointer::ButtonState::Pressed
            }
            input::ButtonState::Released => wl_pointer::ButtonState::Released,
        };
        self.input_state
            .pointer
            .clone()
            .button(button, state, serial, evt.time(), self);

        {
            if evt.state() == input::ButtonState::Pressed {
                let under = self.surface_under(self.input_state.pointer_location);

                if self.input_state.modifiers_state.logo {
                    if let Some((surface, _)) = under {
                        let pointer = self.input_state.pointer.clone();
                        let seat = self.seat.clone();

                        // Check that this surface has a click grab.
                        if pointer.has_grab(serial) {
                            let start_data = pointer.grab_start_data().unwrap();

                            if let Some(space) = self.find_workspace_by_surface_mut(&surface) {
                                if let Some(window) = space.find_window(&surface) {
                                    let toplevel = window.toplevel();

                                    if let Some(res) =
                                        space.move_request(&toplevel, &seat, serial, &start_data)
                                    {
                                        if let Some(window) = space.unmap_toplevel(&toplevel) {
                                            self.grabed_window = Some(window);

                                            let grab = MoveSurfaceGrab {
                                                start_data,
                                                toplevel,
                                                initial_window_location: res
                                                    .initial_window_location,
                                            };
                                            pointer.set_grab(grab, serial);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        if let Some(button) = evt.button() {
            for w in self.visible_workspaces_mut() {
                w.on_pointer_button(button, evt.state());
            }
        }
    }

    fn on_pointer_axis<I: InputBackend>(&mut self, evt: I::PointerAxisEvent) {
        let source = match evt.source() {
            input::AxisSource::Continuous => wl_pointer::AxisSource::Continuous,
            input::AxisSource::Finger => wl_pointer::AxisSource::Finger,
            input::AxisSource::Wheel | input::AxisSource::WheelTilt => {
                wl_pointer::AxisSource::Wheel
            }
        };
        let horizontal_amount = evt
            .amount(input::Axis::Horizontal)
            .unwrap_or_else(|| evt.amount_discrete(input::Axis::Horizontal).unwrap() * 3.0);
        let vertical_amount = evt
            .amount(input::Axis::Vertical)
            .unwrap_or_else(|| evt.amount_discrete(input::Axis::Vertical).unwrap() * 3.0);
        let horizontal_amount_discrete = evt.amount_discrete(input::Axis::Horizontal);
        let vertical_amount_discrete = evt.amount_discrete(input::Axis::Vertical);

        {
            let mut frame = AxisFrame::new(evt.time()).source(source);
            if horizontal_amount != 0.0 {
                frame = frame.value(wl_pointer::Axis::HorizontalScroll, horizontal_amount);
                if let Some(discrete) = horizontal_amount_discrete {
                    frame = frame.discrete(wl_pointer::Axis::HorizontalScroll, discrete as i32);
                }
            } else if source == wl_pointer::AxisSource::Finger {
                frame = frame.stop(wl_pointer::Axis::HorizontalScroll);
            }
            if vertical_amount != 0.0 {
                frame = frame.value(wl_pointer::Axis::VerticalScroll, vertical_amount);
                if let Some(discrete) = vertical_amount_discrete {
                    frame = frame.discrete(wl_pointer::Axis::VerticalScroll, discrete as i32);
                }
            } else if source == wl_pointer::AxisSource::Finger {
                frame = frame.stop(wl_pointer::Axis::VerticalScroll);
            }
            self.input_state.pointer.clone().axis(frame, self);
        }
    }

    fn on_pointer_move(&mut self, time: u32) {
        let serial = SCOUNTER.next_serial();

        for (id, w) in self.workspaces.iter_mut() {
            w.on_pointer_move(self.input_state.pointer_location);

            if w.geometry()
                .contains(self.input_state.pointer_location.to_i32_round())
            {
                self.active_workspace = Some(id.clone());
            }
        }

        let under = self.surface_under(self.input_state.pointer_location);
        self.input_state.pointer.clone().motion(
            self.input_state.pointer_location,
            under,
            serial,
            time,
            self,
        );
    }

    fn clamp_coords(&self, pos: Point<f64, Logical>) -> Point<f64, Logical> {
        if self.output_map.is_empty() {
            return pos;
        }

        let (pos_x, pos_y) = pos.into();
        let output_map = &self.output_map;
        let max_x = output_map.width();
        let clamped_x = pos_x.max(0.0).min(max_x as f64);
        let max_y = output_map.height(clamped_x as i32);

        if let Some(max_y) = max_y {
            let clamped_y = pos_y.max(0.0).min(max_y as f64);

            (clamped_x, clamped_y).into()
        } else {
            (clamped_x, pos_y).into()
        }
    }
}

/// Possible results of a keyboard action
#[derive(Debug)]
enum KeyAction {
    /// Quit the compositor
    Quit,
    /// Trigger a vt-switch
    VtSwitch(i32),
    /// Switch the current screen
    Workspace(usize),
    MoveToWorkspace(usize),
    /// Do nothing more
    None,
}

fn process_keyboard_shortcut(modifiers: ModifiersState, keysym: Keysym) -> Option<KeyAction> {
    if modifiers.logo && keysym == xkb::KEY_q {
        Some(KeyAction::Quit)
    } else if (xkb::KEY_XF86Switch_VT_1..=xkb::KEY_XF86Switch_VT_12).contains(&keysym) {
        // VTSwicth
        Some(KeyAction::VtSwitch(
            (keysym - xkb::KEY_XF86Switch_VT_1 + 1) as i32,
        ))
    } else if modifiers.logo && keysym >= xkb::KEY_1 && keysym <= xkb::KEY_9 {
        Some(KeyAction::Workspace((keysym - xkb::KEY_1) as usize + 1))
    } else if modifiers.logo && modifiers.shift && keysym >= xkb::KEY_1 && keysym <= xkb::KEY_9 {
        Some(KeyAction::MoveToWorkspace((keysym - xkb::KEY_1) as usize))
    } else {
        None
    }
}

impl Anodium {
    fn shortcut_handler(&mut self, action: KeyAction) {
        match action {
            KeyAction::None => {}
            KeyAction::Quit => {
                info!("Quitting.");
                self.running.store(false, Ordering::SeqCst);
            }
            KeyAction::VtSwitch(vt) => {
                info!("Trying to switch to vt {}", vt);
                self.session.change_vt(vt).ok();
            }
            // KeyAction::MoveToWorkspace(num) => {
            // let mut window_map = self.window_map.borrow_mut();
            // }
            // TODO:
            KeyAction::Workspace(num) => {
                self.switch_workspace(&format!("{}", num));
            }
            action => {
                warn!("Key action {:?} unsupported on winit backend.", action);
            }
        }
    }
}
