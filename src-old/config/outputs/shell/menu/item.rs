pub use calloop::channel::Sender;
pub use egui::Ui;

pub use crate::config::eventloop::ConfigEvent;
pub use crate::config::outputs::shell::widget::Widget;

pub trait MenuItem {
    fn render(&self, ui: &mut Ui, config_tx: &Sender<ConfigEvent>);
}

impl std::fmt::Debug for Box<dyn MenuItem> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "derp")
    }
}

impl Clone for Box<dyn MenuItem> {
    fn clone(&self) -> Box<dyn MenuItem> {
        self.to_owned()
    }
}
