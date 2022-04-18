use smithay::{
    desktop,
    utils::{Logical, Rectangle},
    wayland::output,
    wayland::output::Output as SmithayOutput,
};

use anodium_backend::{BackendHandler, BackendState, NewOutputDescriptor, OutputHandler, OutputId};

use crate::{output_manager::Output, state::Anodium};

impl OutputHandler for Anodium {
    fn output_created(&mut self, desc: NewOutputDescriptor) {
        let (output, _) = SmithayOutput::new(
            &mut self.display.borrow_mut(),
            desc.name,
            desc.physical_properties,
            None,
        );

        output.change_current_state(Some(desc.prefered_mode), Some(desc.transform), None, None);
        let id = desc.id;
        output.user_data().insert_if_missing(|| id);

        let output = Output::new(
            output,
            &mut self.anodium_protocol,
            desc.possible_modes,
            self.config_tx.clone(),
        );

        info!("OutputCreated: {}", output.name());
        self.output_manager.add(&output);

        self.config.output_new(output.clone());
        self.config.output_rearrange();
    }

    fn output_mode_updated(&mut self, output_id: &OutputId, mode: output::Mode) {
        let outputs = self.output_manager.outputs();

        let output = outputs
            .iter()
            .find(|o| o.user_data().get::<OutputId>() == Some(output_id));

        if let Some(output) = output {
            output.change_current_state(Some(mode), None, None, None);

            desktop::layer_map_for_output(output).arrange();
        }
    }

    fn output_render(
        &mut self,
        renderer: &mut smithay::backend::renderer::gles2::Gles2Renderer,
        output_id: &OutputId,
        age: usize,
        pointer_image: Option<&smithay::backend::renderer::gles2::Gles2Texture>,
    ) -> Result<Option<Vec<Rectangle<i32, Logical>>>, smithay::backend::SwapBuffersError> {
        let output = {
            let outputs = self.output_manager.outputs();
            outputs
                .iter()
                .find(|o| o.user_data().get::<OutputId>() == Some(output_id))
                .cloned()
                .unwrap()
        };

        self.render(renderer, &output, age, pointer_image)
    }
}

impl BackendHandler for Anodium {
    fn backend_state(&mut self) -> &mut BackendState {
        &mut self.backend
    }

    fn send_frames(&mut self) {
        let time = self.start_time.elapsed().as_millis() as u32;

        self.region_manager.send_frames(time);
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
            self.xwayland.start().ok();
        }
    }

    fn close_compositor(&mut self) {
        self.loop_signal.stop();
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
