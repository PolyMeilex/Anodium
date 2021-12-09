use anodium_protocol::client::AnodiumOutputEvent;

fn main() {
    let context = glib::MainContext::default();
    let l = glib::MainLoop::new(None, false);

    println!("{:?}", context.is_owner());

    anodium_protocol::client::glib::init((), |output, _ddata| {
        println!("New Output: {:?}", output);

        output.init(|output_event, _ddata| match output_event {
            AnodiumOutputEvent::NewWorkspace(workspace) => {
                println!("New Workspace: {:?}", workspace);
                workspace.init(|event, _ddata| {
                    println!("New Workspace Event: {:?}", event);
                });
            }
            AnodiumOutputEvent::Name(name) => {
                println!("Output Name: {:?}", name);
            }
        })
    });

    l.run();
}
