use anodium_protocol::server::AnodiumProtocol;
use smithay::{
    desktop,
    reexports::wayland_server::Display,
    utils::{Logical, Rectangle},
    wayland::output,
};
use std::{cell::RefCell, rc::Rc, sync::atomic::Ordering};

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
        self.output_manager.add(&mut self.workspace, &output);

        self.config.output_new(output.clone());

        if let Some(layout) = self
            .config
            .output_rearrange(self.output_manager.outputs().clone())
        {
            for (output, pos) in self.output_manager.outputs().iter().zip(layout.iter()) {
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
        age: usize,
        pointer_image: Option<&smithay::backend::renderer::gles2::Gles2Texture>,
    ) -> Result<Option<Vec<Rectangle<i32, Logical>>>, smithay::backend::SwapBuffersError> {
        self.render(renderer, output, age, pointer_image)
    }
}

impl BackendHandler for Anodium {
    fn anodium_protocol(&mut self) -> &mut AnodiumProtocol {
        &mut self.anodium_protocol
    }

    fn wl_display(&mut self) -> Rc<RefCell<Display>> {
        self.display.clone()
    }

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
