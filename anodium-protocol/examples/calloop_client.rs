use anodium_protocol::client::AnodiumOutputEvent;
use calloop::EventLoop;

fn main() {
    let mut ev: EventLoop<()> = EventLoop::try_new().unwrap();

    anodium_protocol::client::calloop::init(ev.handle(), |output, _ddata| {
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
    })
    .unwrap();

    ev.run(None, &mut (), |_| {}).unwrap();
}
