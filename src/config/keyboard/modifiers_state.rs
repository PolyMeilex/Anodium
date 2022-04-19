use smithay::wayland::seat::ModifiersState;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct ConfigModifiersState {
    /// The "control" key
    pub ctrl: bool,
    /// The "alt" key
    pub alt: bool,
    /// The "shift" key
    pub shift: bool,
    /// The "logo" key
    ///
    /// Also known as the "windows" key on most keyboards
    pub logo: bool,
}

impl From<ModifiersState> for ConfigModifiersState {
    fn from(m: ModifiersState) -> Self {
        Self {
            ctrl: m.ctrl,
            alt: m.alt,
            shift: m.shift,
            logo: m.logo,
        }
    }
}

impl FromIterator<String> for ConfigModifiersState {
    fn from_iter<T: IntoIterator<Item = String>>(iter: T) -> Self {
        let mut modifiers = Self::default();

        for m in iter.into_iter() {
            match m.as_str() {
                "ctrl" => modifiers.ctrl = true,
                "alt" => modifiers.alt = true,
                "shift" => modifiers.shift = true,
                "logo" => modifiers.logo = true,
                _ => {}
            }
        }

        modifiers
    }
}
