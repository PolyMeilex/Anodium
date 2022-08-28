use std::path::PathBuf;

use anodium_backend::PreferedBackend;
use clap::Parser;

/// Rust wayland compositor
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct AnodiumCliOptions {
    /// Selected backend: auto, x11, winit, udev
    #[clap(short, long, default_value = "auto")]
    pub backend: PreferedBackend,
    /// Path of anodium config
    #[clap(short, long, default_value = "./config.rhai")]
    pub config: PathBuf,
}
