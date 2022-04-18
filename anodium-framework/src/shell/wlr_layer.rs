use smithay::{desktop::LayerSurface, wayland::shell::wlr_layer::LayerShellRequest};

use super::ShellHandler;

impl<D> super::Inner<D>
where
    D: ShellHandler,
{
    pub fn wlr_layer_shell_request(&mut self, request: LayerShellRequest, handler: &mut D) {
        match request {
            LayerShellRequest::NewLayerSurface {
                surface,
                output,
                layer,
                namespace,
            } => {
                let surface = LayerSurface::new(surface, namespace.clone());

                // TODO: Wait for first commit
                self.layers.push(surface.clone());

                handler.layer_created(surface, output, layer, namespace);
            }
            LayerShellRequest::AckConfigure { surface, configure } => {
                handler.layer_ack_configure(surface, configure);
            }
        }
    }
}
