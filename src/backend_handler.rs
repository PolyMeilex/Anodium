use smithay::desktop;
use std::sync::atomic::Ordering;

use crate::{framework::backend::BackendEvent, state::Anodium};

impl Anodium {
    pub fn handle_backend_event(&mut self, event: BackendEvent) {
        match event {
            BackendEvent::RequestOutputConfigure { output } => {
                self.config.output_new(output);
            }
            BackendEvent::OutputCreated { output } => {
                info!("OutputCreated: {}", output.name());

                self.output_map.add(&mut self.workspace, &output);
            }
            BackendEvent::OutputModeUpdate { output } => {
                let mut map = desktop::layer_map_for_output(output);
                map.arrange();
            }
            BackendEvent::OutputRender {
                renderer: frame,
                output,
                pointer_image,
            } => {
                self.render(frame, output, pointer_image).ok();
            }
            BackendEvent::SendFrames => {
                let time = self.start_time.elapsed().as_millis() as u32;

                self.workspace.send_frames(false, time);
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
