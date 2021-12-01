use std::{
    io::{BufRead, BufReader},
    os::unix::{
        net::{UnixListener, UnixStream},
        prelude::AsRawFd,
    },
};

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use smithay::reexports::{
    calloop::{self, RegistrationToken},
    nix,
};

use calloop::{channel::Sender, generic::Generic, Interest, LoopHandle, Mode, PostAction};

use crate::{state::Anodium, utils::LogResult};

#[derive(Debug)]
enum ClientEvent {
    Input(u64, String),
    Closed(u64),
}

fn client_listener(
    event_loop: &LoopHandle<'static, Anodium>,
    tx: Sender<ClientEvent>,
    stream: UnixStream,
) -> Option<RegistrationToken> {
    let fd = stream.as_raw_fd();

    let id = {
        let mut hasher = DefaultHasher::new();

        let c =
            nix::sys::socket::getsockopt(fd, nix::sys::socket::sockopt::PeerCredentials).unwrap();
        c.pid().hash(&mut hasher);
        c.uid().hash(&mut hasher);
        c.gid().hash(&mut hasher);

        hasher.finish()
    };

    event_loop
        .insert_source(
            Generic::from_fd(fd, Interest::READ, Mode::Edge),
            move |_, _, _state| {
                let mut reader = BufReader::new(&stream);

                let mut buffer = String::new();
                let len = reader
                    .read_line(&mut buffer)
                    .log_err("Ipc Read:")
                    .unwrap_or(0);

                Ok(if len == 0 {
                    tx.send(ClientEvent::Closed(id)).unwrap();
                    PostAction::Remove
                } else {
                    tx.send(ClientEvent::Input(id, buffer)).unwrap();
                    PostAction::Continue
                })
            },
        )
        .ok()
}

pub fn ipc_listener(event_loop: LoopHandle<'static, Anodium>) {
    let path = "./anodium-ipc.sock";

    std::fs::remove_file(path).ok();

    let listener = UnixListener::bind(path).unwrap();
    let fd = listener.as_raw_fd();

    let (tx, rx) = calloop::channel::channel::<ClientEvent>();

    event_loop
        .insert_source(Generic::from_fd(fd, Interest::READ, Mode::Edge), {
            let event_loop = event_loop.clone();
            move |_, _, _state| {
                let (stream, _) = listener.accept().unwrap();

                client_listener(&event_loop, tx.clone(), stream);

                Ok(PostAction::Continue)
            }
        })
        .unwrap();

    event_loop
        .insert_source(rx, {
            |event, _, _state| {
                error!("\nEvent: {:?}", event);
            }
        })
        .unwrap();
}
