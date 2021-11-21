use std::{
    cell::{RefCell, RefMut},
    collections::HashMap,
    os::unix::net::UnixStream,
    rc::Rc,
    sync::Arc,
};

use smithay::{
    reexports::wayland_server::Client,
    utils::{x11rb::X11Source, Logical, Point},
};

use x11rb::{
    connection::Connection as _,
    protocol::{
        composite::{ConnectionExt as _, Redirect},
        xproto::{ChangeWindowAttributesAux, ConnectionExt as _, EventMask, Window, WindowClass},
    },
    rust_connection::{DefaultStream, RustConnection},
};

x11rb::atom_manager! {
    pub Atoms: AtomsCookie {
        WM_S0,
        WL_SURFACE_ID,
        _ANVIL_CLOSE_CONNECTION,
    }
}

/// The actual runtime state of the XWayland integration.
pub struct X11State {
    pub conn: Arc<RustConnection>,
    pub atoms: Atoms,
    pub unpaired_surfaces: HashMap<u32, (Window, Point<i32, Logical>)>,
}

impl X11State {
    pub fn start_wm(
        connection: UnixStream,
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
        let wm = Self {
            conn: Arc::clone(&conn),
            atoms,
            unpaired_surfaces: Default::default(),
        };

        Ok((
            wm,
            X11Source::new(
                conn,
                win,
                atoms._ANVIL_CLOSE_CONNECTION,
                slog_scope::logger(),
            ),
        ))
    }

    pub fn get_mut(client: &Client) -> Option<RefMut<Self>> {
        if let Some(x11) = client.data_map().get::<Rc<RefCell<X11State>>>() {
            let x11 = x11.borrow_mut();
            Some(x11)
        } else {
            None
        }
    }
}
