use std::{cell::RefCell, ops::Deref, rc::Rc};

use wayland_server::{Display, Filter, Global, Main};

/// Generated interfaces for the protocol
pub mod protocol {
    #![allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables)]
    #![allow(non_upper_case_globals, non_snake_case, unused_imports)]
    #![allow(missing_docs, clippy::all)]

    use wayland_commons::map::{Object, ObjectMetadata};
    use wayland_commons::smallvec;
    use wayland_commons::wire::{Argument, ArgumentType, Message, MessageDesc};
    use wayland_commons::{Interface, MessageGroup};
    use wayland_server::*;
    include!(concat!(env!("OUT_DIR"), "/server_api.rs"));
}

use protocol::anodium_workspace_manager::AnodiumWorkspaceManager;

mod output;
mod workspace;

pub use output::AnodiumProtocolOutput;
pub use workspace::AnodiumProtocolWorkspace;

#[derive(Default)]
struct Inner {
    known: Vec<AnodiumWorkspaceManager>,
    outputs: Vec<AnodiumProtocolOutput>,
}

pub struct AnodiumProtocol {
    inner: Rc<RefCell<Inner>>,
}

impl AnodiumProtocol {
    pub fn init(display: &mut Display) -> (AnodiumProtocol, Global<AnodiumWorkspaceManager>) {
        let inner = Rc::new(RefCell::new(Inner::default()));

        let global: Global<AnodiumWorkspaceManager> = display.create_global(1, {
            let inner = inner.clone();
            Filter::new(
                move |(manager, _): (Main<AnodiumWorkspaceManager>, _), _, _| {
                    manager.quick_assign(|_res, _, _| {});
                    manager.assign_destructor(Filter::new({
                        let inner = inner.clone();
                        move |res: AnodiumWorkspaceManager, _, _| {
                            inner
                                .borrow_mut()
                                .known
                                .retain(|m| !m.as_ref().equals(res.as_ref()));
                        }
                    }));

                    let mut inner = inner.borrow_mut();
                    for output in inner.outputs.iter_mut() {
                        output.new_instance(&manager)
                    }

                    inner.known.push(manager.deref().clone());
                },
            )
        });

        (AnodiumProtocol { inner }, global)
    }

    pub fn new_output(&mut self) -> AnodiumProtocolOutput {
        let mut inner = self.inner.borrow_mut();

        let mut output = AnodiumProtocolOutput::new();

        for res in inner.known.iter() {
            output.new_instance(res);
        }

        inner.outputs.push(output.clone());

        output
    }
}
