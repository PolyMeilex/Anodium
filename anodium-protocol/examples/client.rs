use anodium_protocol::client::{anodium_output, anodium_workspace_manager};
use wayland_client::{protocol::wl_seat, Display, GlobalManager};

// A minimal example printing the list of globals advertised by the server and
// then exiting

fn main() {
    // Connect to the server
    let display = Display::connect_to_env().unwrap();

    let mut event_queue = display.create_event_queue();

    let attached_display = (*display).clone().attach(event_queue.token());

    // We use the GlobalManager convenience provided by the crate, it covers
    // most classic use cases and avoids us the trouble to manually implement
    // the registry
    let globals = GlobalManager::new(&attached_display);

    // A roundtrip synchronization to make sure the server received our registry
    // creation and sent us the global list
    event_queue
        .sync_roundtrip(&mut (), |_, _, _| unreachable!())
        .unwrap();

    // Print the list
    for (id, interface, version) in globals.list() {
        println!("{}: {} (version {})", id, interface, version);
    }

    let anodium_workspace_manger = globals
        .instantiate_exact::<anodium_workspace_manager::AnodiumWorkspaceManager>(1)
        .expect("Compositor does not support anodium protocol")
        .quick_assign(|manager, event, _| {
            println!("{:?}", event);

            match event {
                anodium_workspace_manager::Event::Output { output } => {
                    output.quick_assign(|output, event, _| {
                        println!("{:?}", event);

                        match event {
                            anodium_output::Event::Workspace { workspace } => {
                                workspace.quick_assign(|workspace, event, _| {
                                    println!("{:?}", event);
                                });
                            }
                            _ => {}
                        };
                    });
                }
                _ => {}
            }
            //
        });

    event_queue
        .sync_roundtrip(&mut (), |_, _, _| { /* we ignore unfiltered messages */ })
        .unwrap();

    loop {
        event_queue
            .dispatch(&mut (), |event, _, _| {
                println!("{:?}", event);
            })
            .unwrap();
    }
}
