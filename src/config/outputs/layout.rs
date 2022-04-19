use smithay::wayland::output::Mode;

#[derive(Debug, Default, Clone)]
pub struct OutputLayout(pub Vec<OutputDescriptor>);

impl OutputLayout {
    pub fn find_output(&self, name: &str) -> Option<&OutputDescriptor> {
        self.0.iter().find(|o| o.name() == name)
    }

    pub fn iter(&self) -> std::slice::Iter<OutputDescriptor> {
        self.0.iter()
    }
}

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
