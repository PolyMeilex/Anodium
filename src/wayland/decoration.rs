use smithay::reexports::{
    wayland_protocols::unstable::xdg_decoration::v1::server::zxdg_decoration_manager_v1::{
        self, ZxdgDecorationManagerV1,
    },
    wayland_protocols::unstable::xdg_decoration::v1::server::zxdg_toplevel_decoration_v1::{
        Mode, ZxdgToplevelDecorationV1,
    },
    wayland_server::{Display, Filter, Global, Main},
};

#[allow(unused)]
pub fn init_decoration_manager(display: &mut Display) -> Global<ZxdgDecorationManagerV1> {
    display.create_global(
        1,
        Filter::new(
            move |(manager, _version): (Main<ZxdgDecorationManagerV1>, _), _, _| {
                manager.quick_assign(move |_manager, request, _| {
                    match request {
                        zxdg_decoration_manager_v1::Request::Destroy => {
                            // All is handled by destructor.
                        }
                        zxdg_decoration_manager_v1::Request::GetToplevelDecoration { id, .. } => {
                            id.configure(Mode::ServerSide);
                            id.quick_assign(move |id, _request, _| {
                                id.configure(Mode::ServerSide);
                            });

                            id.assign_destructor(Filter::new(
                                move |_decoration: ZxdgToplevelDecorationV1, _, _| {},
                            ));
                        }

                        _ => unreachable!(),
                    }
                });

                manager.assign_destructor(Filter::new(move |_manager: ZxdgDecorationManagerV1, _, _| {}));
            },
        ),
    )
}
