use std::{cell::RefCell, ops::Deref, rc::Rc};

use wayland_server::{Filter, Main};

use super::protocol::{anodium_output::AnodiumOutput, anodium_workspace::AnodiumWorkspace};

#[derive(Default)]
struct Inner {
    name: String,
    known: Vec<AnodiumWorkspace>,
}

#[derive(Clone)]
pub struct AnodiumProtocolWorkspace {
    inner: Rc<RefCell<Inner>>,
}

impl AnodiumProtocolWorkspace {
    pub(super) fn new() -> Self {
        Self {
            inner: Default::default(),
        }
    }

    pub(super) fn new_instance(&mut self, output: &AnodiumOutput) {
        let mut inner = self.inner.borrow_mut();

        let output = output.as_ref().client().and_then(|client| {
            let workspace: Main<AnodiumWorkspace> = client.create_resource(1)?;
            workspace.quick_assign(|_res, _, _| {});
            workspace.assign_destructor(Filter::new({
                let inner = self.inner.clone();
                move |res: AnodiumWorkspace, _, _| {
                    inner.borrow_mut().known.retain(|w| w != &res);
                }
            }));

            output.workspace(workspace.deref());

            workspace.name(inner.name.clone());

            Some(workspace.deref().clone())
        });

        if let Some(output) = output {
            inner.known.push(output);
        }
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
