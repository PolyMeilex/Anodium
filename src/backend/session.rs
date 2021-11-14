use smithay::backend::session::{
    auto::{AutoSession, Error},
    Session,
};
use smithay::reexports::nix::fcntl::OFlag;
use std::os::unix::prelude::RawFd;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct AnodiumSession {
    udev: Option<AutoSession>,
}

impl AnodiumSession {
    pub fn new_udev(s: AutoSession) -> Self {
        Self { udev: Some(s) }
    }

    pub fn new_winit() -> Self {
        Self { udev: None }
    }
}

impl Session for AnodiumSession {
    type Error = Error;

    fn open(&mut self, path: &Path, flags: OFlag) -> Result<RawFd, Self::Error> {
        if let Some(s) = self.udev.as_mut() {
            s.open(path, flags)
        } else {
            unimplemented!("Winit Session can't open devices");
        }
    }

    fn close(&mut self, fd: RawFd) -> Result<(), Self::Error> {
        if let Some(s) = self.udev.as_mut() {
            s.close(fd)
        } else {
            Ok(())
        }
    }

    fn change_vt(&mut self, vt: i32) -> Result<(), Self::Error> {
        if let Some(s) = self.udev.as_mut() {
            s.change_vt(vt)
        } else {
            Ok(())
        }
    }

    fn is_active(&self) -> bool {
        if let Some(s) = self.udev.as_ref() {
            s.is_active()
        } else {
            true
        }
    }

    fn seat(&self) -> String {
        if let Some(s) = self.udev.as_ref() {
            s.seat()
        } else {
            String::from("winit")
        }
    }
}
