/// Generated interfaces for the protocol
pub mod server {
    #![allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables)]
    #![allow(non_upper_case_globals, non_snake_case, unused_imports)]
    #![allow(missing_docs, clippy::all)]

    use wayland_commons::map::{Object, ObjectMetadata};
    use wayland_commons::smallvec;
    use wayland_commons::wire::{Argument, ArgumentType, Message, MessageDesc};
    use wayland_commons::{Interface, MessageGroup};
    use wayland_server::*;
    include!(concat!(env!("OUT_DIR"), "/server_api.rs"));
}

/// Generated interfaces for the protocol
pub mod client {
    #![allow(dead_code, non_camel_case_types, unused_unsafe, unused_variables)]
    #![allow(non_upper_case_globals, non_snake_case, unused_imports)]
    #![allow(missing_docs, clippy::all)]

    use wayland_client::*;
    use wayland_commons::map::{Object, ObjectMetadata};
    use wayland_commons::smallvec;
    use wayland_commons::wire::{Argument, ArgumentType, Message, MessageDesc};
    use wayland_commons::{Interface, MessageGroup};
    include!(concat!(env!("OUT_DIR"), "/client_api.rs"));
}
