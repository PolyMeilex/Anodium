use std::sync::atomic::Ordering;

use anodium_backend::{BackendRequest, InputHandler};

use crate::{output_manager::Output, region_manager::Region, Anodium};

use smithay::{
    backend::input::{
        self, ButtonState, Event, InputBackend, InputEvent, KeyState, KeyboardKeyEvent,
        PointerAxisEvent, PointerButtonEvent, PointerMotionAbsoluteEvent, PointerMotionEvent,
    },
    desktop::WindowSurfaceType,
    reexports::wayland_server::protocol::wl_pointer,
    utils::{Logical, Point},
    wayland::{
        output::Output as SmithayOutput,
        seat::{keysyms as xkb, AxisFrame, FilterResult, Keysym, ModifiersState},
        SERIAL_COUNTER as SCOUNTER,
    },
};

impl InputHandler for Anodium {
    fn process_input_event<I: InputBackend>(
        &mut self,
        event: InputEvent<I>,
        absolute_output: Option<&SmithayOutput>,
    ) {
        let absolute_output = absolute_output.map(|o| Output::wrap(o.clone()));

        let captured = match &event {
            InputEvent::Keyboard { event, .. } => {
                let action = self.keyboard_key_to_action::<I>(event);
                if action == KeyAction::Filtred {
                    true
                } else if action != KeyAction::None {
                    self.shortcut_handler(action);
                    self.input_state.borrow().keyboard.is_focused()
                } else {
                    true
                }
            }
            InputEvent::PointerMotion { event, .. } => {
                let pointer_location;
                {
                    let mut input_state = self.input_state.borrow_mut();
                    input_state.pointer_location =
                        self.clamp_coords(input_state.pointer_location + event.delta());
                    pointer_location = input_state.pointer_location;
                }

                if let Some(region) = self.region_manager.region_under(pointer_location) {
                    let region_pos = region.position().to_f64();
                    self.on_pointer_move(event.time(), region_pos);
                    self.surface_under(self.input_state.borrow().pointer_location)
                        .is_none()
                } else {
                    false
                }
            }
            InputEvent::PointerMotionAbsolute { event, .. } => {
                if let Some(absolute_output) = absolute_output {
                    if let Some(region) = self.region_manager.find_output_region(&absolute_output) {
                        let workspace = region.active_workspace();
                        let output_geometry =
                            workspace.space().output_geometry(&absolute_output).unwrap();
                        let output_pos = output_geometry.loc.to_f64();
                        let region_pos = region.position().to_f64();
                        let output_size = output_geometry.size;

                        self.input_state.borrow_mut().pointer_location =
                            event.position_transformed(output_size) + output_pos + region_pos;
                        self.on_pointer_move(event.time(), region_pos);
                        self.surface_under(self.input_state.borrow().pointer_location)
                            .is_none()
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
            InputEvent::PointerButton { event, .. } => {
                self.on_pointer_button::<I>(event);
                self.surface_under(self.input_state.borrow().pointer_location)
                    .is_none()
            }
            InputEvent::PointerAxis { event, .. } => {
                self.on_pointer_axis::<I>(event);
                self.surface_under(self.input_state.borrow().pointer_location)
                    .is_none()
            }
            _ => false,
        };
        let pointer_location = self.input_state.borrow().pointer_location;
        if let Some(region) = self.region_manager.region_under(pointer_location) {
            if let Some(output) = region
                .active_workspace()
                .space()
                .output_under(pointer_location - region.position().to_f64())
                .next()
            {
                let output = &Output::wrap(output.clone());
                if captured {
                    self.process_egui_event(event, &region, output);
                } else {
                    self.reset_egui_event(output);
                }
            } else {
                error!("output under egui not found!");
            }
        }
    }
}

impl Anodium {
    fn reset_egui_event(&self, output: &Output) {
        let mut max_point = Point::default();
        max_point.x = i32::MAX;
        max_point.y = i32::MAX;
        output.egui().handle_pointer_motion(max_point);
    }

    fn process_egui_event<I: InputBackend>(
        &self,
        event: InputEvent<I>,
        region: &Region,
        output: &Output,
    ) {
        match event {
            InputEvent::PointerMotion { .. } | InputEvent::PointerMotionAbsolute { .. } => {
                let output_loc = region
                    .active_workspace()
                    .space()
                    .output_geometry(output)
                    .unwrap()
                    .loc;
                let mouse_location = self.input_state.borrow().pointer_location
                    - output_loc.to_f64()
                    - region.position().to_f64();

                output
                    .egui()
                    .handle_pointer_motion(mouse_location.to_i32_round());
            }

            InputEvent::PointerButton { event, .. } => {
                if let Some(button) = event.button() {
                    output.egui().handle_pointer_button(
                        button,
                        event.state() == ButtonState::Pressed,
                        self.input_state.borrow().modifiers_state,
                    );
                }
            }

            //InputEvent::Keyboard { event } => {
            //TODO - is that enough or do we need the whole code from here https://github.com/Smithay/smithay-egui/blob/main/examples/integrate.rs#L69 ?
            // output.egui().handle_keyboard(
            //     event.key_code(),
            //     event.state() == KeyState::Pressed,
            //     self.input_state.modifiers_state,
            // );
            //}
            InputEvent::PointerAxis { event, .. } => output.egui().handle_pointer_axis(
                event
                    .amount_discrete(input::Axis::Horizontal)
                    .or_else(|| event.amount(input::Axis::Horizontal).map(|x| x * 3.0))
                    .unwrap_or(0.0)
                    * 10.0,
                event
                    .amount_discrete(input::Axis::Vertical)
                    .or_else(|| event.amount(input::Axis::Vertical).map(|x| x * 3.0))
                    .unwrap_or(0.0)
                    * 10.0,
            ),
            _ => {}
        }
    }
}

impl Anodium {
    fn keyboard_key_to_action<I: InputBackend>(&mut self, evt: &I::KeyboardKeyEvent) -> KeyAction {
        let keycode = evt.key_code();
        let state = evt.state();
        debug!("key"; "keycode" => keycode, "state" => format!("{:?}", state));
        let serial = SCOUNTER.next_serial();
        let time = Event::time(evt);

        let mut input_state = self.input_state.borrow_mut();
        let configvm = self.config.clone();

        input_state
            .keyboard
            .clone()
            .input(keycode, state, serial, time, |modifiers, handle| {
                let keysym = handle.modified_sym();

                if let KeyState::Pressed = state {
                    input_state.pressed_keys.insert(keysym);
                } else {
                    input_state.pressed_keys.remove(&keysym);
                }

                let keysym_desc = ::xkbcommon::xkb::keysym_get_name(keysym);

                debug!( "keysym";
                    "state" => format!("{:?}", state),
                    "mods" => format!("{:?}", modifiers),
                    "keysym" => &keysym_desc
                );
                input_state.modifiers_state = *modifiers;

                // If the key is pressed and triggered a action
                // we will not forward the key to the client.
                // Additionally add the key to the suppressed keys
                // so that we can decide on a release if the key
                // should be forwarded to the client or not.

                if let KeyState::Pressed = state {
                    let action = process_keyboard_shortcut(*modifiers, keysym);

                    if action.is_some() {
                        input_state.suppressed_keys.push(keysym);
                    } else if configvm.key_action(keysym, state, &input_state.pressed_keys) {
                        input_state.suppressed_keys.push(keysym);
                        return FilterResult::Intercept(KeyAction::Filtred);
                    }

                    action
                        .map(FilterResult::Intercept)
                        .unwrap_or(FilterResult::Forward)
                } else {
                    let suppressed = input_state.suppressed_keys.contains(&keysym);
                    if suppressed {
                        input_state.suppressed_keys.retain(|k| *k != keysym);
                        FilterResult::Intercept(KeyAction::Filtred)
                    } else {
                        FilterResult::Forward
                    }
                }
            })
            .unwrap_or(KeyAction::None)
    }

    pub fn clear_keyboard_focus(&mut self) {
        let serial = SCOUNTER.next_serial();
        self.input_state
            .borrow_mut()
            .keyboard
            .set_focus(None, serial);
    }

    fn on_pointer_button<I: InputBackend>(&mut self, evt: &I::PointerButtonEvent) {
        let serial = SCOUNTER.next_serial();

        debug!("Mouse Event"; "Mouse button" => format!("{:?}", evt.button()));
        let input_state_clone = self.input_state.clone();
        let input_state = input_state_clone.borrow();
        let button = evt.button_code();
        let state = match evt.state() {
            input::ButtonState::Pressed => {
                // change the keyboard focus unless the pointer is grabbed
                if !input_state.pointer.is_grabbed() {
                    let point = input_state.pointer_location;
                    // let under = self.surface_under(self.input_state.pointer_location);
                    if let Some(region) = self.region_manager.region_under(point) {
                        let window = region
                            .active_workspace()
                            .space()
                            .window_under(point - region.position().to_f64())
                            .cloned();
                        // let surface = under.as_ref().map(|&(ref s, _)| s);
                        // if let Some(surface) = surface {
                        //     let mut window = None;
                        //     if let Some(space) = self.find_workspace_by_surface_mut(surface) {
                        //         window = space.find_window(surface).cloned();
                        //     }
                        //     self.update_focused_window(window);
                        // }

                        self.update_focused_window(window.as_ref());

                        let surface = window
                            .and_then(|w| {
                                w.surface_under(
                                    point - region.position().to_f64(),
                                    WindowSurfaceType::ALL,
                                )
                            })
                            .map(|s| s.0);

                        input_state.keyboard.set_focus(surface.as_ref(), serial);
                    } else {
                        error!("got a button press without a region under it, this shouldn't be possible: {:?}", point);
                    }
                }
                wl_pointer::ButtonState::Pressed
            }
            input::ButtonState::Released => wl_pointer::ButtonState::Released,
        };
        input_state
            .pointer
            .clone()
            .button(button, state, serial, evt.time(), self);

        // {
        //     if evt.state() == input::ButtonState::Pressed {
        //         let under = self.surface_under(self.input_state.pointer_location);

        //         if self.input_state.modifiers_state.logo {
        //             if let Some((surface, _)) = under {
        //                 let pointer = self.input_state.pointer.clone();
        //                 let seat = self.seat.clone();

        //                 // Check that this surface has a click grab.
        //                 if pointer.has_grab(serial) {
        //                     let start_data = pointer.grab_start_data().unwrap();

        //                     if let Some(space) = self.find_workspace_by_surface_mut(&surface) {
        //                         if let Some(window) = space.find_window(&surface) {
        //                             let toplevel = window.toplevel();

        //                             if let Some(res) =
        //                                 space.move_request(&toplevel, &seat, serial, &start_data)
        //                             {
        //                                 if let Some(window) = space.unmap_toplevel(&toplevel) {
        //                                     self.grabed_window = Some(window);

        //                                     let grab = MoveSurfaceGrab {
        //                                         start_data,
        //                                         toplevel,
        //                                         initial_window_location: res
        //                                             .initial_window_location,
        //                                     };
        //                                     pointer.set_grab(grab, serial);
        //                                 }
        //                             }
        //                         }
        //                     }
        //                 }
        //             }
        //         }
        //     }
        // }

        // if let Some(button) = evt.button() {
        //     for w in self.visible_workspaces_mut() {
        //         w.on_pointer_button(button, evt.state());
        //     }
        // }
    }

    fn on_pointer_axis<I: InputBackend>(&mut self, evt: &I::PointerAxisEvent) {
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
            let input_state = self.input_state.clone();
            input_state.borrow().pointer.clone().axis(frame, self);
        }
    }

    fn on_pointer_move(&mut self, time: u32, offset: Point<f64, Logical>) {
        let serial = SCOUNTER.next_serial();

        // for (id, w) in self.workspaces.iter_mut() {
        //     w.on_pointer_move(self.input_state.pointer_location);

        //     if w.geometry()
        //         .contains(self.input_state.pointer_location.to_i32_round())
        //     {
        //         self.active_workspace = Some(id.clone());
        //     }
        // }
        let input_state = self.input_state.clone();
        let input_state = input_state.borrow();
        let under = self.surface_under(input_state.pointer_location);
        input_state.pointer.clone().motion(
            input_state.pointer_location - offset,
            under,
            serial,
            time,
            self,
        );
    }

    fn clamp_coords(&self, pos: Point<f64, Logical>) -> Point<f64, Logical> {
        self.region_manager.clamp_coords(pos)
    }
}

/// Possible results of a keyboard action
#[derive(Debug, PartialEq, Eq)]
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
    /// Do nothing more
    Filtred,
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
            KeyAction::None | KeyAction::Filtred => {}
            KeyAction::Quit => {
                info!("Quitting.");
                self.running.store(false, Ordering::SeqCst);
            }
            KeyAction::VtSwitch(vt) => {
                info!("Trying to switch to vt {}", vt);
                // self.session.change_vt(vt).ok();
                // TODO(poly)
                self.backend_tx.send(BackendRequest::ChangeVT(vt)).ok();
            }
            // KeyAction::MoveToWorkspace(num) => {
            // let mut window_map = self.window_map.borrow_mut();
            // }
            // TODO:
            // KeyAction::Workspace(_num) => {
            // self.switch_workspace(&format!("{}", num));
            // }
            action => {
                warn!("Key action {:?} unsupported on winit backend.", action);
            }
        }
    }
}
