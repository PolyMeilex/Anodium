use std::sync::atomic::Ordering;

use crate::{framework::backend::BackendEvent, positioner::universal::Universal, state::Anodium};

impl Anodium {
    pub fn handle_backend_event(&mut self, event: BackendEvent) {
        match event {
            BackendEvent::OutputCreated { mut output } => {
                self.config.output_new(output.clone());

                info!("OutputCreated: {}", output.name());
                let id = self.workspaces.len() + 1;
                let id = format!("{}", id);

                if self.active_workspace.is_none() {
                    self.active_workspace = Some(id.clone());
                }

                output.set_active_workspace(id.clone());
                self.output_map.add(output);

                let positioner = Universal::new(Default::default(), Default::default());

                self.workspaces.insert(id, Box::new(positioner));
                self.update_workspaces_geometry();
            }
            BackendEvent::OutputModeUpdate { output } => {
                let space = self.workspaces.get_mut(&output.active_workspace()).unwrap();
                space.set_geometry(output.usable_geometry());

                self.output_map.rearrange();
                self.update_workspaces_geometry();
            }
            BackendEvent::OutputRender {
                frame,
                output,
                pointer_image,
            } => {
                self.render(frame, output, pointer_image).ok();
            }
            BackendEvent::SendFrames => {
                let time = self.start_time.elapsed().as_millis() as u32;

                for w in self.visible_workspaces() {
                    w.send_frames(time);
                }
                self.output_map.send_frames(time);
            }
            BackendEvent::StartCompositor => {
                self.start();
            }
            BackendEvent::CloseCompositor => {
                self.running.store(false, Ordering::SeqCst);
            }
        }
    }
}
