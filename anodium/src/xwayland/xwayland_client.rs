use std::{os::unix::net::UnixStream, sync::Arc};

use smithay::{reexports::wayland_server::Client, utils::x11rb::X11Source};
use x11rb::{
    connection::Connection,
    protocol::{
        composite::{ConnectionExt as _, Redirect},
        xproto::{ChangeWindowAttributesAux, ConnectionExt as _, EventMask, WindowClass},
    },
    rust_connection::{DefaultStream, RustConnection},
};

x11rb::atom_manager! {
    pub Atoms: AtomsCookie {
        WM_S0,
        WL_SURFACE_ID,
        _ANODIUM_CLOSE_CONNECTION,
    }
}

#[derive(Debug, Clone)]
pub struct XWaylandClient {
    pub conn: Arc<RustConnection>,
    pub atoms: Atoms,
    pub wl_client: Client,
}

impl XWaylandClient {
    pub fn start(
        connection: UnixStream,
        client: Client,
    ) -> Result<(Self, X11Source), Box<dyn std::error::Error>> {
        // Create an X11 connection. XWayland only uses screen 0.
        let screen = 0;
        let stream = DefaultStream::from_unix_stream(connection)?;
        let conn = RustConnection::connect_to_stream(stream, screen)?;
        let atoms = Atoms::new(&conn)?.reply()?;

        let screen = &conn.setup().roots[0];

        // Actually become the WM by redirecting some operations
        conn.change_window_attributes(
            screen.root,
            &ChangeWindowAttributesAux::default().event_mask(EventMask::SUBSTRUCTURE_REDIRECT),
        )?;

        // Tell XWayland that we are the WM by acquiring the WM_S0 selection. No X11 clients are accepted before this.
        let win = conn.generate_id()?;
        conn.create_window(
            screen.root_depth,
            win,
            screen.root,
            // x, y, width, height, border width
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            x11rb::COPY_FROM_PARENT,
            &Default::default(),
        )?;
        conn.set_selection_owner(win, atoms.WM_S0, x11rb::CURRENT_TIME)?;

        // XWayland wants us to do this to function properly...?
        conn.composite_redirect_subwindows(screen.root, Redirect::MANUAL)?;

        conn.flush()?;

        let conn = Arc::new(conn);

        let xwayland = Self {
            conn: Arc::clone(&conn),
            atoms,
            wl_client: client,
        };

        let source = X11Source::new(
            conn,
            win,
            atoms._ANODIUM_CLOSE_CONNECTION,
            slog_scope::logger(),
        );

        Ok((xwayland, source))
    }
}
