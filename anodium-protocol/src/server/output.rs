use std::{cell::RefCell, ops::Deref, rc::Rc};

use wayland_server::{Filter, Main};

use super::{
    protocol::{anodium_output::AnodiumOutput, anodium_workspace_manager::AnodiumWorkspaceManager},
    workspace::AnodiumProtocolWorkspace,
};

#[derive(Default)]
struct Inner {
    name: String,
    workspaces: Vec<AnodiumProtocolWorkspace>,
    known: Vec<AnodiumOutput>,
}

#[derive(Clone)]
pub struct AnodiumProtocolOutput {
    inner: Rc<RefCell<Inner>>,
}

impl AnodiumProtocolOutput {
    pub(super) fn new() -> Self {
        Self {
            inner: Default::default(),
        }
    }

    pub(super) fn new_instance(&mut self, manager: &AnodiumWorkspaceManager) {
        let mut inner = self.inner.borrow_mut();

        let output = manager.as_ref().client().and_then(|client| {
            let output: Main<AnodiumOutput> = client.create_resource(1)?;
            output.quick_assign(|_res, _, _| {});
            output.assign_destructor(Filter::new({
                let inner = self.inner.clone();
                move |res: AnodiumOutput, _, _| {
                    inner.borrow_mut().known.retain(|o| o != &res);
                }
            }));

            manager.output(output.deref());

            output.name(inner.name.clone());

            for ws in inner.workspaces.iter_mut() {
                ws.new_instance(&output);
            }

            Some(output.deref().clone())
        });

        if let Some(output) = output {
            inner.known.push(output);
        }
    }

    pub fn new_workspace(&mut self) -> AnodiumProtocolWorkspace {
        let mut inner = self.inner.borrow_mut();

        let mut workspace = AnodiumProtocolWorkspace::new();

        for res in inner.known.iter() {
            workspace.new_instance(res);
        }

        inner.workspaces.push(workspace.clone());

        workspace
    }

    pub fn set_name<S: AsRef<str>>(&mut self, name: S) {
        let mut inner = self.inner.borrow_mut();
        let name = name.as_ref();

        inner.name = name.to_owned();

        for res in inner.known.iter() {
            res.name(name.to_owned())
        }
    }
}
