use std::path::PathBuf;

use gumdrop::Options;

use strum::EnumString;

#[derive(Debug, EnumString)]
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

#[derive(Debug, Options)]
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
