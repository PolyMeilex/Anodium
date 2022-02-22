use std::path::PathBuf;

use gumdrop::Options;

use strum::EnumString;

use anodium_backend::PreferedBackend;

#[derive(Debug, Clone, EnumString)]
pub enum Backend {
    #[strum(serialize = "auto")]
    Auto,
    #[strum(serialize = "x11")]
    X11,
    #[strum(serialize = "winit")]
    Winit,
    #[strum(serialize = "udev")]
    Udev,
}

impl From<Backend> for PreferedBackend {
    fn from(from: Backend) -> Self {
        match from {
            Backend::Auto => PreferedBackend::Auto,
            Backend::X11 => PreferedBackend::X11,
            Backend::Winit => PreferedBackend::Winit,
            Backend::Udev => PreferedBackend::Udev,
        }
    }
}

#[derive(Debug, Clone, Options)]
pub struct AnodiumOptions {
    #[options(help = "print help message")]
    help: bool,

    #[options(
        help = "selected backend: auto, x11, winit, udev",
        meta = "BACKEND",
        default = "auto"
    )]
    pub backend: Backend,

    #[options(
        help = "use provided path as rhai config script",
        meta = "PATH",
        default = "./config.rhai"
    )]
    pub config: PathBuf,
}

pub fn get_anodium_options() -> AnodiumOptions {
    AnodiumOptions::parse_args_default_or_exit()
}
