#[cfg(feature = "winit")]
mod winit;

#[cfg(feature = "udev")]
mod udev;

use std::{process::Command, sync::atomic::Ordering};

use crate::{backend::Backend, MainState};

use smithay::{
    backend::input::{
        self, Event, InputBackend, InputEvent, KeyState, KeyboardKeyEvent, PointerAxisEvent,
        PointerButtonEvent, PointerMotionAbsoluteEvent, PointerMotionEvent,
    },
    reexports::wayland_server::protocol::wl_pointer,
    utils::{Logical, Point},
    wayland::{
        seat::{keysyms as xkb, AxisFrame, Keysym, ModifiersState},
        SERIAL_COUNTER as SCOUNTER,
    },
};

impl MainState {
    pub fn process_input_event<B: Backend, I: InputBackend>(
        &mut self,
        backend: &mut B,
        event: InputEvent<I>,
    ) {
        match event {
            InputEvent::Keyboard { event, .. } => match self.keyboard_key_to_action::<I>(event) {
                action => self.shortcut_handler(backend, action),
            },
            InputEvent::PointerMotion { event, .. } => {
                self.set_pointer_location(self.pointer_location() + event.delta());
                self.set_pointer_location(self.clamp_coords(self.pointer_location()));
                self.on_pointer_move(event.time());
            }
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let output_size = self
                    .desktop_layout
                    .borrow()
                    .output_map
                    .find_by_name(crate::backend::winit::OUTPUT_NAME)
                    .map(|o| o.size());

                if let Some(output_size) = output_size {
                    self.set_pointer_location(event.position_transformed(output_size));
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
        debug!(self.log, "key"; "keycode" => keycode, "state" => format!("{:?}", state));
        let serial = SCOUNTER.next_serial();
        let log = &self.log;
        let time = Event::time(&evt);
        let mut action = KeyAction::None;
        let suppressed_keys = &mut self.suppressed_keys;
        self.keyboard
            .input(keycode, state, serial, time, |modifiers, keysym| {
                debug!(log, "keysym";
                    "state" => format!("{:?}", state),
                    "mods" => format!("{:?}", modifiers),
                    "keysym" => ::xkbcommon::xkb::keysym_get_name(keysym)
                );

                // If the key is pressed and triggered a action
                // we will not forward the key to the client.
                // Additionally add the key to the suppressed keys
                // so that we can decide on a release if the key
                // should be forwarded to the client or not.
                if let KeyState::Pressed = state {
                    action = process_keyboard_shortcut(*modifiers, keysym);

                    // forward to client only if action == KeyAction::Forward
                    let forward = matches!(action, KeyAction::Forward);

                    if !forward {
                        suppressed_keys.push(keysym);
                    }

                    forward
                } else {
                    let suppressed = suppressed_keys.contains(&keysym);

                    if suppressed {
                        suppressed_keys.retain(|k| *k != keysym);
                    }

                    !suppressed
                }
            });
        action
    }

    fn on_pointer_button<I: InputBackend>(&mut self, evt: I::PointerButtonEvent) {
        let serial = SCOUNTER.next_serial();
        let button = match evt.button() {
            input::MouseButton::Left => 0x110,
            input::MouseButton::Right => 0x111,
            input::MouseButton::Middle => 0x112,
            input::MouseButton::Other(b) => b as u32,
        };
        let state = match evt.state() {
            input::ButtonState::Pressed => {
                // change the keyboard focus unless the pointer is grabbed
                if !self.pointer.is_grabbed() {
                    let pointer_location = self.pointer_location();

                    let under = self.desktop_layout.borrow().surface_under(pointer_location);

                    self.keyboard
                        .set_focus(under.as_ref().map(|&(ref s, _)| s), serial);
                }
                wl_pointer::ButtonState::Pressed
            }
            input::ButtonState::Released => wl_pointer::ButtonState::Released,
        };
        self.pointer.button(button, state, serial, evt.time());

        for w in self.desktop_layout.borrow_mut().visible_workspaces_mut() {
            w.on_pointer_button(evt.button(), evt.state());
        }
    }

    fn on_pointer_axis<I: InputBackend>(&mut self, evt: I::PointerAxisEvent) {
        let source = match evt.source() {
            input::AxisSource::Continuous => wl_pointer::AxisSource::Continuous,
            input::AxisSource::Finger => wl_pointer::AxisSource::Finger,
            input::AxisSource::Wheel | input::AxisSource::WheelTilt => wl_pointer::AxisSource::Wheel,
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
            self.pointer.axis(frame);
        }
    }

    fn on_pointer_move(&mut self, time: u32) {
        let serial = SCOUNTER.next_serial();

        let pointer_location = self.pointer_location();

        self.desktop_layout.borrow_mut().on_pointer_move(pointer_location);

        let under = self.desktop_layout.borrow().surface_under(pointer_location);
        self.pointer.motion(pointer_location, under, serial, time);
    }

    fn clamp_coords(&self, pos: Point<f64, Logical>) -> Point<f64, Logical> {
        if self.desktop_layout.borrow().output_map.is_empty() {
            return pos;
        }

        let (pos_x, pos_y) = pos.into();
        let output_map = &self.desktop_layout.borrow().output_map;
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
    /// run a command
    Run(String),
    /// Switch the current screen
    Workspace(usize),
    MoveToWorkspace(usize),
    /// Forward the key to the client
    Forward,
    /// Do nothing more
    None,
}

fn process_keyboard_shortcut(modifiers: ModifiersState, keysym: Keysym) -> KeyAction {
    if modifiers.logo && keysym == xkb::KEY_q {
        KeyAction::Quit
    } else if (xkb::KEY_XF86Switch_VT_1..=xkb::KEY_XF86Switch_VT_12).contains(&keysym) {
        // VTSwicth
        KeyAction::VtSwitch((keysym - xkb::KEY_XF86Switch_VT_1 + 1) as i32)
    } else if modifiers.logo && keysym == xkb::KEY_Return {
        // run terminal
        KeyAction::Run("yavt".into())
        // KeyAction::Run("alacritty".into())
        // KeyAction::Run("weston-terminal".into())
    } else if modifiers.logo && keysym >= xkb::KEY_1 && keysym <= xkb::KEY_9 {
        KeyAction::Workspace((keysym - xkb::KEY_1) as usize)
    } else if modifiers.logo && modifiers.shift && keysym >= xkb::KEY_1 && keysym <= xkb::KEY_9 {
        KeyAction::MoveToWorkspace((keysym - xkb::KEY_1) as usize)
    } else {
        KeyAction::Forward
    }
}

impl MainState {
    fn shortcut_handler<B: Backend>(&mut self, backend: &mut B, action: KeyAction) {
        match action {
            KeyAction::None | KeyAction::Forward => {}
            KeyAction::Quit => {
                info!(self.log, "Quitting.");
                self.running.store(false, Ordering::SeqCst);
            }
            KeyAction::VtSwitch(vt) => {
                info!(self.log, "Trying to switch to vt {}", vt);
                backend.change_vt(vt);
            }
            KeyAction::Run(cmd) => {
                info!(self.log, "Starting program"; "cmd" => cmd.clone());
                if let Err(e) = Command::new(&cmd).spawn() {
                    error!(self.log,
                        "Failed to start program";
                        "cmd" => cmd,
                        "err" => format!("{:?}", e)
                    );
                }
            }
            // KeyAction::MoveToWorkspace(num) => {
            // let mut window_map = self.window_map.borrow_mut();
            // }
            // TODO:
            // KeyAction::Workspace(num) => {
            // let mut window_map = self.window_map.borrow_mut();
            // window_map.workspace = num % 5;
            // }
            action => {
                warn!(self.log, "Key action {:?} unsupported on winit backend.", action);
            }
        }
    }
}
