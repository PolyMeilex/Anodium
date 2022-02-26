use smithay::{
    desktop,
    utils::{Logical, Rectangle},
    wayland::output,
    wayland::output::Output as SmithayOutput,
};
use std::sync::atomic::Ordering;

use anodium_backend::{BackendHandler, OutputHandler};

use crate::{output_manager::Output, state::Anodium};

impl OutputHandler for Anodium {
    fn output_created(&mut self, output: SmithayOutput, possible_modes: Vec<output::Mode>) {
        let output = Output::new(
            output,
            &mut self.anodium_protocol,
            possible_modes,
            self.config_tx.clone(),
        );

        info!("OutputCreated: {}", output.name());
        self.output_manager.add(&output);

        self.config.output_new(output.clone());
        self.config.output_rearrange();
    }

    fn output_mode_updated(&mut self, output: &SmithayOutput, mode: output::Mode) {
        output.change_current_state(Some(mode), None, None, None);

        desktop::layer_map_for_output(output).arrange();
    }

    fn output_render(
        &mut self,
        renderer: &mut smithay::backend::renderer::gles2::Gles2Renderer,
        output: &SmithayOutput,
        age: usize,
        pointer_image: Option<&smithay::backend::renderer::gles2::Gles2Texture>,
    ) -> Result<Option<Vec<Rectangle<i32, Logical>>>, smithay::backend::SwapBuffersError> {
        let output = Output::wrap(output.clone());
        self.render(renderer, &output, age, pointer_image)
    }
}

impl BackendHandler for Anodium {
    fn send_frames(&mut self) {
        let time = self.start_time.elapsed().as_millis() as u32;

        self.region_manager.send_frames(false, time);
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
