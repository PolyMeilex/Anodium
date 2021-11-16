use smithay::{
    reexports::wayland_server::DispatchData, wayland::shell::wlr_layer::LayerShellRequest,
};

use super::ShellEvent;

impl super::Inner {
    pub fn wlr_layer_shell_request(&mut self, request: LayerShellRequest, ddata: DispatchData) {
        match request {
            LayerShellRequest::NewLayerSurface {
                surface,
                output,
                layer,
                namespace,
            } => {
                (self.cb)(
                    ShellEvent::LayerCreated {
                        surface,
                        output,
                        layer,
                        namespace,
                    },
                    ddata,
                );
            }
            LayerShellRequest::AckConfigure { surface, configure } => {
                (self.cb)(ShellEvent::LayerAckConfigure { surface, configure }, ddata);
            }
        }
    }
}
