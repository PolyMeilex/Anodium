use std::{
    cell::RefCell,
    collections::HashSet,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

use anodium_protocol::server::AnodiumProtocol;
use calloop::channel::{self, Channel};
use smithay::{
    backend::renderer::gles2::{Gles2Renderer, Gles2Texture},
    desktop::{
        self,
        space::{DynamicRenderElements, SurfaceTree},
    },
    reexports::{
        calloop::{self, channel::Sender, generic::Generic, Interest, LoopHandle, PostAction},
        wayland_server::{protocol::wl_surface::WlSurface, Display},
    },
    utils::{Logical, Point, Rectangle},
    wayland::{
        data_device::{self, DataDeviceEvent},
        output::xdg::init_xdg_output_manager,
        seat::{CursorImageStatus, KeyboardHandle, ModifiersState, PointerHandle, Seat, XkbConfig},
        shm::init_shm_global,
    },
};

#[cfg(feature = "xwayland")]
use smithay::xwayland::{XWayland, XWaylandEvent};

use crate::{
    cli::AnodiumOptions,
    config::{eventloop::ConfigEvent, ConfigVM},
    framework::backend::BackendRequest,
    framework::{cursor::PointerElement, shell::ShellManager},
    output_manager::{Output, OutputManager},
    render,
    workspace::Workspace,
};

pub struct InputState {
    pub pointer_location: Point<f64, Logical>,
    pub previous_pointer_location: Point<f64, Logical>,
    pub pointer: PointerHandle,

    pub keyboard: KeyboardHandle,
    pub modifiers_state: ModifiersState,

    pub suppressed_keys: Vec<u32>,
    pub pressed_keys: HashSet<u32>,
}

pub struct Anodium {
    pub handle: LoopHandle<'static, Self>,

    pub running: Arc<AtomicBool>,
    pub display: Rc<RefCell<Display>>,

    pub shell_manager: ShellManager<Self>,

    pub dnd_icon: Arc<Mutex<Option<WlSurface>>>,
    pub cursor_status: Arc<Mutex<CursorImageStatus>>,

    pub input_state: InputState,

    pub seat: Seat,

    pub options: AnodiumOptions,

    pub start_time: std::time::Instant,
    last_update: Instant,

    pub config: ConfigVM,

    // Desktop
    pub anodium_protocol: AnodiumProtocol,
    pub output_manager: OutputManager,

    pub workspace: Workspace,

    pub active_workspace: Option<String>,
    pub focused_window: Option<desktop::Window>,

    #[cfg(feature = "xwayland")]
    pub xwayland: XWayland<Self>,

    pub backend_tx: Sender<BackendRequest>,
    pub config_tx: Sender<ConfigEvent>,
}

impl Anodium {
    /// init the wayland connection
    fn init_wayland_connection(handle: &LoopHandle<'static, Self>, display: &Rc<RefCell<Display>>) {
        handle
            .insert_source(
                Generic::from_fd(
                    display.borrow().get_poll_fd(),
                    Interest::READ,
                    calloop::Mode::Level,
                ),
                |_, _, state: &mut Self| {
                    let display = state.display.clone();
                    let mut display = display.borrow_mut();
                    match display.dispatch(std::time::Duration::from_millis(0), state) {
                        Ok(_) => Ok(PostAction::Continue),
                        Err(e) => {
                            error!("I/O error on the Wayland display: {}", e);
                            state.running.store(false, Ordering::SeqCst);
                            Err(e)
                        }
                    }
                },
            )
            .expect("Failed to init the wayland event source.");
    }

    /// init the xwayland connection
    #[cfg(feature = "xwayland")]
    fn init_xwayland_connection(
        handle: &LoopHandle<'static, Self>,
        display: &Rc<RefCell<Display>>,
    ) -> XWayland<Self> {
        let (xwayland, channel) =
            XWayland::new(handle.clone(), display.clone(), slog_scope::logger());

        let ret = handle.insert_source(channel, {
            let handle = handle.clone();
            move |event, _, state| match event {
                XWaylandEvent::Ready { connection, client } => state
                    .shell_manager
                    .xwayland_ready(&handle, connection, client),
                XWaylandEvent::Exited => {
                    error!("Xwayland crashed");
                }
            }
        });
        if let Err(e) = ret {
            error!(
                "Failed to insert the XWaylandSource into the event loop: {}",
                e
            );
        }
        xwayland
    }

    /// init data device
    fn init_data_device(display: &Rc<RefCell<Display>>) -> Arc<Mutex<Option<WlSurface>>> {
        let dnd_icon = Arc::new(Mutex::new(None));

        data_device::init_data_device(
            &mut display.borrow_mut(),
            {
                let dnd_icon = dnd_icon.clone();
                move |event| match event {
                    DataDeviceEvent::DnDStarted { icon, .. } => {
                        *dnd_icon.lock().unwrap() = icon;
                    }
                    DataDeviceEvent::DnDDropped => {
                        *dnd_icon.lock().unwrap() = None;
                    }
                    _ => {}
                }
            },
            data_device::default_action_chooser,
            slog_scope::logger(),
        );

        dnd_icon
    }

    /// init wayland seat, keyboard and pointer
    fn init_seat(
        display: &Rc<RefCell<Display>>,
        seat_name: String,
    ) -> (
        Seat,
        PointerHandle,
        KeyboardHandle,
        Arc<Mutex<CursorImageStatus>>,
    ) {
        let (mut seat, _) = Seat::new(&mut display.borrow_mut(), seat_name, slog_scope::logger());

        let cursor_status = Arc::new(Mutex::new(CursorImageStatus::Default));

        let pointer = seat.add_pointer({
            let cursor_status = cursor_status.clone();
            move |new_status| *cursor_status.lock().unwrap() = new_status
        });

        let keyboard = seat
            .add_keyboard(XkbConfig::default(), 200, 25, |seat, focus| {
                data_device::set_data_device_focus(seat, focus.and_then(|s| s.as_ref().client()))
            })
            .expect("Failed to initialize the keyboard");

        (seat, pointer, keyboard, cursor_status)
    }

    fn init_config_channel(handle: &LoopHandle<'static, Self>) -> Sender<ConfigEvent> {
        let (sender, reciver) = calloop::channel::channel::<ConfigEvent>();

        use calloop::channel::Event;
        handle
            .insert_source(reciver, |event, _metadata, state: &mut Anodium| {
                if let Event::Msg(event) = event {
                    state.process_config_event(event);
                }
            })
            .unwrap();

        sender
    }

    pub fn new(
        handle: LoopHandle<'static, Self>,
        seat_name: String,
        options: AnodiumOptions,
    ) -> (Self, Channel<BackendRequest>) {
        let log = slog_scope::logger();

        let (backend_tx, backend_rx) = channel::channel();

        let display = Rc::new(RefCell::new(Display::new()));

        // init the wayland connection
        Self::init_wayland_connection(&handle, &display);

        // Init the basic compositor globals

        init_shm_global(&mut display.borrow_mut(), vec![], log.clone());
        init_xdg_output_manager(&mut display.borrow_mut(), log.clone());

        let dnd_icon = Self::init_data_device(&display);

        let shell_manager = ShellManager::init_shell(&mut display.borrow_mut());

        let (seat, pointer, keyboard, cursor_status) = Self::init_seat(&display, seat_name);

        let (anodium_protocol, _global) = AnodiumProtocol::init(&mut display.borrow_mut());

        #[cfg(feature = "xwayland")]
        let xwayland = Self::init_xwayland_connection(&handle, &display);

        let config_tx = Self::init_config_channel(&handle);
        let output_map = OutputManager::new();

        let config = ConfigVM::new(
            config_tx.clone(),
            output_map.clone(),
            handle.clone(),
            options.config.clone(),
        )
        .unwrap();

        (
            Self {
                handle,

                running: Arc::new(AtomicBool::new(true)),

                shell_manager,
                display,

                dnd_icon,
                cursor_status,

                input_state: InputState {
                    pointer_location: (0.0, 0.0).into(),
                    previous_pointer_location: (0.0, 0.0).into(),
                    pointer,
                    keyboard,
                    modifiers_state: Default::default(),
                    suppressed_keys: Vec::new(),
                    pressed_keys: HashSet::new(),
                },

                seat,

                options,

                start_time: Instant::now(),
                last_update: Instant::now(),

                config,

                anodium_protocol,
                output_manager: output_map,
                workspace: Workspace::new(),

                active_workspace: None,
                focused_window: Default::default(),

                #[cfg(feature = "xwayland")]
                xwayland,
                backend_tx,
                config_tx,
            },
            backend_rx,
        )
    }
}

impl Anodium {
    pub fn update(&mut self) {
        self.shell_manager.refresh();
        self.workspace.refresh();

        if let Some(focused_window) = &self.focused_window {
            if !focused_window.toplevel().alive() {
                self.update_focused_window(None);
            }
        }

        self.last_update = Instant::now();
    }

    // draw the custom cursor if applicable
    fn prepare_cursor_element(
        &self,
        relative_location: Point<i32, Logical>,
    ) -> Option<SurfaceTree> {
        // draw the cursor as relevant
        let mut cursor_status = self.cursor_status.lock().unwrap();
        // reset the cursor if the surface is no longer alive
        let mut reset = false;
        if let CursorImageStatus::Image(ref surface) = *cursor_status {
            reset = !surface.as_ref().is_alive();
        }
        if reset {
            *cursor_status = CursorImageStatus::Default;
        }

        if let CursorImageStatus::Image(ref wl_surface) = *cursor_status {
            let elm = render::draw_cursor(wl_surface.clone(), relative_location);
            Some(elm)
        } else {
            None
        }
    }

    // draw the dnd icon if applicable
    fn prepare_dnd_element(&self, relative_location: Point<i32, Logical>) -> Option<SurfaceTree> {
        let guard = self.dnd_icon.lock().unwrap();
        guard.as_ref().and_then(|wl_surface| {
            if wl_surface.as_ref().is_alive() {
                Some(render::draw_dnd_icon(wl_surface.clone(), relative_location))
            } else {
                None
            }
        })
    }

    pub fn render(
        &mut self,
        renderer: &mut Gles2Renderer,
        output: &Output,
        age: usize,
        pointer_image: Option<&Gles2Texture>,
    ) -> Result<Option<Vec<Rectangle<i32, Logical>>>, smithay::backend::SwapBuffersError> {
        let output_geometry = self.workspace.output_geometry(output).unwrap();

        let mut elems: Vec<DynamicRenderElements<_>> = Vec::new();

        let frame = output.render_egui_shell(
            &self.start_time,
            &self.input_state.modifiers_state,
            &self.config_tx,
        );
        elems.push(Box::new(frame));

        // Pointer Related:
        if output_geometry
            .to_f64()
            .contains(self.input_state.pointer_location)
        {
            let (ptr_x, ptr_y) = self.input_state.pointer_location.into();
            let relative_location =
                Point::<i32, Logical>::from((ptr_x as i32, ptr_y as i32)) - output_geometry.loc;

            if let Some(wl_cursor) = self.prepare_cursor_element(relative_location) {
                elems.push(Box::new(wl_cursor));
            } else if let Some(texture) = pointer_image {
                elems.push(Box::new(PointerElement::new(
                    texture.clone(),
                    relative_location,
                    self.input_state.pointer_location != self.input_state.previous_pointer_location,
                )));
            }
            self.input_state.previous_pointer_location = self.input_state.pointer_location;

            if let Some(wl_dnd) = self.prepare_dnd_element(output_geometry.loc) {
                elems.push(Box::new(wl_dnd));
            }
        }

        let render_result = self
            .workspace
            .render_output(renderer, output, age, [0.1, 0.1, 0.1, 1.0], &elems)
            .unwrap();

        if render_result.is_some() {
            #[cfg(feature = "debug")]
            output.tick_fps();
        }

        Ok(render_result)
    }
}

impl Anodium {
    pub fn update_focused_window(&mut self, window: Option<&desktop::Window>) {
        self.workspace.windows().for_each(|w| {
            w.set_activated(false);
        });

        if let Some(window) = window {
            window.set_activated(true);
        }

        self.workspace.windows().for_each(|w| w.configure());

        self.focused_window = window.cloned();
    }
}
