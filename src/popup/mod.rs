use smithay::desktop::PopupKind;

#[derive(Debug)]
pub struct Popup {
    pub popup: PopupKind,
}

impl std::ops::Deref for Popup {
    type Target = PopupKind;

    fn deref(&self) -> &Self::Target {
        &self.popup
    }
}

impl std::ops::DerefMut for Popup {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.popup
    }
}
