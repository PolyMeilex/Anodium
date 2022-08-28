use wayland_client::{DispatchData, GlobalManager, Main};

/// Generated interfaces for the protocol
pub mod protocol {
    #![allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables)]
    #![allow(non_upper_case_globals, non_snake_case, unused_imports)]
    #![allow(missing_docs, clippy::all)]

    use wayland_client::*;
    use wayland_commons::{
        map::{Object, ObjectMetadata},
        smallvec,
        wire::{Argument, ArgumentType, Message, MessageDesc},
        Interface, MessageGroup,
    };
    include!(concat!(env!("OUT_DIR"), "/client_api.rs"));
}

use protocol::{anodium_output, anodium_workspace, anodium_workspace_manager};

#[cfg(feature = "calloop-adapter")]
pub mod calloop;

#[cfg(feature = "glib-adapter")]
pub mod glib;

#[derive(Debug)]
pub enum AnodiumWorkspaceEvent {
    Name(String),
}

#[derive(Debug)]
pub struct AnodiumWorkspace {
    res: Main<anodium_workspace::AnodiumWorkspace>,
}

impl AnodiumWorkspace {
    pub fn new(res: Main<anodium_workspace::AnodiumWorkspace>) -> Self {
        Self { res }
    }

    pub fn init<F>(self, cb: F)
    where
        F: Fn(AnodiumWorkspaceEvent, DispatchData) + 'static,
    {
        self.res.quick_assign(move |_workspace, event, ddata| {
            let event = match event {
                anodium_workspace::Event::Name { name } => AnodiumWorkspaceEvent::Name(name),
            };

            cb(event, ddata);
        });
    }
}

#[derive(Debug)]
pub enum AnodiumOutputEvent {
    NewWorkspace(AnodiumWorkspace),
    Name(String),
}

#[derive(Debug)]
pub struct AnodiumOutput {
    res: Main<anodium_output::AnodiumOutput>,
}

impl AnodiumOutput {
    pub fn new(res: Main<anodium_output::AnodiumOutput>) -> Self {
        Self { res }
    }

    pub fn init<F>(self, cb: F)
    where
        F: Fn(AnodiumOutputEvent, DispatchData) + 'static,
    {
        self.res.quick_assign(move |_output, event, ddata| {
            match event {
                anodium_output::Event::Workspace { workspace } => {
                    cb(
                        AnodiumOutputEvent::NewWorkspace(AnodiumWorkspace::new(workspace)),
                        ddata,
                    );
                }
                anodium_output::Event::Name { name } => {
                    cb(AnodiumOutputEvent::Name(name), ddata);
                }
            };
        });
    }
}

pub fn init_global<F>(globals: &GlobalManager, cb: F)
where
    F: Fn(AnodiumOutput, DispatchData) + 'static,
{
    globals
        .instantiate_exact::<anodium_workspace_manager::AnodiumWorkspaceManager>(1)
        .expect("Compositor does not support anodium protocol")
        .quick_assign(move |_manager, event, ddata| match event {
            anodium_workspace_manager::Event::Output { output } => {
                cb(AnodiumOutput::new(output), ddata);
            }
        });
}
