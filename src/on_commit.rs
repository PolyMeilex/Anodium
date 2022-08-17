use crate::State;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

#[derive(Default)]
pub struct OnCommitDispatcher {
    cbs: Vec<(WlSurface, Box<dyn FnOnce(&mut State, &WlSurface)>)>,
}

impl OnCommitDispatcher {
    pub fn on_next_commit<F>(&mut self, surface: WlSurface, cb: F)
    where
        F: FnOnce(&mut State, &WlSurface) + 'static,
    {
        self.cbs.push((surface, Box::new(cb)));
    }

    pub fn handle_commit(state: &mut State, surface: &WlSurface) {
        let mut queue = Vec::new();

        let cbs = &mut state.commit_dispatcher.cbs;

        // Drain filter
        while let Some(id) = cbs.iter().position(|(s, _)| s == surface) {
            let (_, cb) = cbs.remove(id);
            queue.push(cb);
        }

        for cb in queue {
            cb(state, surface);
        }
    }
}
