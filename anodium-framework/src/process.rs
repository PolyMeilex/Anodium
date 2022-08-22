use std::{io::Read, process::Stdio};

use smithay::reexports::calloop::{
    self,
    channel::{self, Channel, ChannelError},
};

pub struct Child {
    source: Channel<String>,
}

impl Child {
    pub fn spawn(cmd: &mut std::process::Command) -> Result<Self, std::io::Error> {
        let mut cmd = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let (tx, source) = channel::channel();

        std::thread::Builder::new()
            .name("Child Stdout reader".to_string())
            .spawn(move || {
                let mut buff = String::new();
                cmd.stdout
                    .take()
                    .unwrap()
                    .read_to_string(&mut buff)
                    .unwrap();
                tx.send(buff).unwrap();
            })?;

        Ok(Self { source })
    }
}

impl calloop::EventSource for Child {
    type Event = String;
    type Metadata = ();
    type Ret = ();
    type Error = ChannelError;

    fn process_events<F>(
        &mut self,
        readiness: calloop::Readiness,
        token: calloop::Token,
        mut callback: F,
    ) -> Result<calloop::PostAction, Self::Error>
    where
        F: FnMut(Self::Event, &mut Self::Metadata) -> Self::Ret,
    {
        self.source
            .process_events(readiness, token, |event, _| match event {
                channel::Event::Msg(output) => {
                    callback(output, &mut ());
                }
                channel::Event::Closed => {}
            })
    }

    fn register(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> calloop::Result<()> {
        self.source.register(poll, token_factory)
    }

    fn reregister(
        &mut self,
        poll: &mut calloop::Poll,
        token_factory: &mut calloop::TokenFactory,
    ) -> calloop::Result<()> {
        self.source.reregister(poll, token_factory)
    }

    fn unregister(&mut self, poll: &mut calloop::Poll) -> calloop::Result<()> {
        self.source.unregister(poll)
    }
}
