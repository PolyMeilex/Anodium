use smithay::wayland::output::Mode;

#[derive(Debug, Clone, serde::Deserialize)]
pub struct OutputDescriptor {
    name: String,
    resolution: [i32; 2],
    refresh: i32,
}

impl OutputDescriptor {
    pub fn mode(&self) -> Mode {
        Mode {
            size: (self.resolution[0], self.resolution[1]).into(),
            refresh: self.refresh,
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}
