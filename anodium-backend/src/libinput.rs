use input::Libinput;
use smithay::{
    backend::{
        input::{InputEvent, KeyboardKeyEvent},
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        session::{libseat::LibSeatSession, Session},
    },
    reexports::calloop::LoopHandle,
};

use crate::InputHandler;

/// Initialize libinput backend
pub fn init<D>(event_loop: LoopHandle<D>, mut session: LibSeatSession)
where
    D: InputHandler,
{
    let mut libinput_context =
        Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(session.clone().into());
    libinput_context.udev_assign_seat(&session.seat()).unwrap();

    let libinput_backend = LibinputInputBackend::new(libinput_context);

    let mut abort_key_combo = AbortKeyCombo::empty();
    let mut modifiers = Modifiers::empty();

    event_loop
        .insert_source(libinput_backend, move |mut event, _, handler| {
            match &mut event {
                InputEvent::DeviceAdded { device } => {
                    device.config_tap_set_enabled(true).ok();
                }
                InputEvent::DeviceRemoved { .. } => {}
                InputEvent::Keyboard { event } => {
                    let pressed = match event.state() {
                        smithay::backend::input::KeyState::Released => false,
                        smithay::backend::input::KeyState::Pressed => true,
                    };

                    let key_code = event.key_code();

                    abort_key_combo.on_key(pressed, key_code);
                    modifiers.on_key(pressed, key_code);

                    if let KEY_F1..=KEY_F10 = key_code {
                        if pressed && modifiers.contains(Modifiers::CTRL | Modifiers::ALT) {
                            let vt = key_code - KEY_F1 + 1;
                            session.change_vt(vt as i32).ok();
                        }
                    }

                    if modifiers.contains(Modifiers::CTRL) && abort_key_combo.is_all() {
                        panic!("Aborted");
                    }
                }
                _ => {}
            }

            handler.process_input_event(event, None);
        })
        .unwrap();
}

const KEY_F1: u32 = 59;
const KEY_F10: u32 = 68;

bitflags::bitflags! {
    struct AbortKeyCombo: u8 {
        const A = 0b00000001;
        const B = 0b00000010;
        const O = 0b00000100;
        const R = 0b00001000;
        const T = 0b00010000;
    }
}

impl AbortKeyCombo {
    pub fn on_key(&mut self, pressed: bool, key_code: u32) {
        const KEY_A: u32 = 30;
        const KEY_B: u32 = 48;
        const KEY_O: u32 = 24;
        const KEY_R: u32 = 19;
        const KEY_T: u32 = 20;

        match key_code {
            KEY_A => self.set(Self::A, pressed),
            KEY_B => self.set(Self::B, pressed),
            KEY_O => self.set(Self::O, pressed),
            KEY_R => self.set(Self::R, pressed),
            KEY_T => self.set(Self::T, pressed),
            _ => {}
        }
    }
}

bitflags::bitflags! {
    struct Modifiers: u8 {
        const CTRL = 0b00000001;
        const ALT = 0b00000010;
    }
}

impl Modifiers {
    pub fn on_key(&mut self, pressed: bool, key_code: u32) {
        const KEY_LEFTCTRL: u32 = 29;
        const KEY_LEFTALT: u32 = 56;

        match key_code {
            KEY_LEFTCTRL => self.set(Self::CTRL, pressed),
            KEY_LEFTALT => self.set(Self::ALT, pressed),
            _ => {}
        }
    }
}
