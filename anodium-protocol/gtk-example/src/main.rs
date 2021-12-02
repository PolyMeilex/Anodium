use gtk::prelude::*;

use anodium_protocol::client::{AnodiumOutputEvent, AnodiumWorkspaceEvent};

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);

    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .build();
    window.add(&root);

    let args = std::env::var("WAYLAND_DISPLAY").unwrap();

    std::env::set_var("WAYLAND_DISPLAY", "wayland-0");

    anodium_protocol::client::glib::init((), move |output, _| {
        let output_root = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .build();
        let label = gtk::Label::builder().label("Output: Unknown").build();

        output_root.add(&label);
        root.add(&output_root);

        output.init(move |output_event, _| match output_event {
            AnodiumOutputEvent::NewWorkspace(workspace) => {
                let label = gtk::Label::builder().label("Workspace: Unknown").build();
                output_root.add(&label);

                workspace.init(move |event, _| match event {
                    AnodiumWorkspaceEvent::Name(name) => {
                        label.set_text(&format!("Workspace: {}", name));
                    }
                });
            }
            AnodiumOutputEvent::Name(name) => {
                label.set_text(&format!("Output: {}", name));
            }
        });
    });

    std::env::set_var("WAYLAND_DISPLAY", &args);

    window.show_all();
}

fn main() {
    let application = gtk::Application::new(
        Some("com.github.polymeilex.anodium.example"),
        Default::default(),
    );

    application.connect_activate(build_ui);

    application.run();
}
