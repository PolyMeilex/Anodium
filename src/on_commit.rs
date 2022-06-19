use crate::State;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

#[derive(Default)]
pub struct OnCommitDispatcher {
    cbs: Vec<Box<dyn FnOnce(&mut State, &WlSurface)>>,
}

impl OnCommitDispatcher {
    pub fn on_next_commit<F>(&mut self, cb: F)
    where
        F: FnOnce(&mut State, &WlSurface) + 'static,
    {
        self.cbs.push(Box::new(cb));
    }

    pub fn handle_commit(state: &mut State, surface: &WlSurface) {
        let queue: Vec<_> = state.on_commit_dispatcher.cbs.drain(..).collect();

        for cb in queue {
            cb(state, surface);
        }
    }
}
