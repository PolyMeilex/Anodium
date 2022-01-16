use smithay::{desktop, wayland::output};
use std::sync::atomic::Ordering;

use crate::{
    framework::backend::{BackendHandler, OutputHandler},
    output_manager::{Output, OutputDescriptor},
    state::Anodium,
};

impl OutputHandler for Anodium {
    fn ask_for_output_mode(
        &mut self,
        desc: &OutputDescriptor,
        modes: &[output::Mode],
    ) -> output::Mode {
        self.config
            .ask_for_output_mode(desc, modes)
            .unwrap_or_else(|| modes[0])
    }

    fn output_created(&mut self, output: crate::output_manager::Output) {
        info!("OutputCreated: {}", output.name());
        self.output_map.add(&mut self.workspace, &output);

        self.config.output_new(output.clone());

        if let Some(layout) = self
            .config
            .output_rearrange(self.output_map.outputs().into())
        {
            for (output, pos) in self.output_map.outputs().iter().zip(layout.iter()) {
                let scale = self.workspace.output_scale(output).unwrap_or(1.0);

                let (x, y) = *pos;
                self.workspace.map_output(output, scale, (x, y));
            }
        }
    }

    fn output_mode_updated(&mut self, output: &crate::output_manager::Output, mode: output::Mode) {
        output.change_current_state(Some(mode), None, None, None);

        desktop::layer_map_for_output(output).arrange();
    }

    fn output_render(
        &mut self,
        renderer: &mut smithay::backend::renderer::gles2::Gles2Renderer,
        output: &Output,
        pointer_image: Option<&smithay::backend::renderer::gles2::Gles2Texture>,
    ) {
        self.render(renderer, output, pointer_image).ok();
    }
}

impl BackendHandler for Anodium {
    fn send_frames(&mut self) {
        let time = self.start_time.elapsed().as_millis() as u32;

        self.workspace.send_frames(false, time);
    }

    fn start_compositor(&mut self) {
        let socket_name = self
            .display
            .borrow_mut()
            .add_socket_auto()
            .unwrap()
            .into_string()
            .unwrap();

        info!("Listening on wayland socket"; "name" => socket_name.clone());
        ::std::env::set_var("WAYLAND_DISPLAY", &socket_name);

        #[cfg(feature = "xwayland")]
        {
            use crate::utils::LogResult;

            self.xwayland
                .start()
                .log_err("Failed to start XWayland:")
                .ok();
        }
    }

    fn close_compositor(&mut self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

// impl Anodium {
//     pub fn handle_backend_event(&mut self, event: BackendEvent) {
//         match event {
//             BackendEvent::RequestOutputConfigure { output } => {
//                 self.config.output_new(output);
//             }
//             BackendEvent::OutputCreated { output } => {
//                 info!("OutputCreated: {}", output.name());

//                 self.output_map.add(&mut self.workspace, &output);
//             }
//             BackendEvent::OutputModeUpdate { output } => {
//                 let mut map = desktop::layer_map_for_output(output);
//                 map.arrange();
//             }
//             BackendEvent::OutputRender {
//                 renderer: frame,
//                 output,
//                 pointer_image,
//             } => {
//                 self.render(frame, output, pointer_image).ok();
//             }
//             BackendEvent::SendFrames => {
//                 let time = self.start_time.elapsed().as_millis() as u32;

//                 self.workspace.send_frames(false, time);
//             }
//             BackendEvent::StartCompositor => {
//                 self.start();
//             }
//             BackendEvent::CloseCompositor => {
//                 self.running.store(false, Ordering::SeqCst);
//             }
//         }
//     }
// }
