use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

use smithay::{
    backend::{
        renderer::{gles2::Gles2Texture, Frame, Transform},
        session::Session,
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
        shell::wlr_layer::Layer,
        shm::init_shm_global,
    },
};

#[cfg(feature = "xwayland")]
use smithay::xwayland::{XWayland, XWaylandEvent};

use crate::{
    config::{eventloop::ConfigEvent, ConfigVM},
    framework::backend::session::AnodiumSession,
    framework::shell::ShellManager,
    output_map::{Output, OutputMap},
    positioner::{universal::Universal, Positioner},
    render::{self, renderer::RenderFrame},
    utils::{AsWlSurface, VisibleWorkspaceIter, VisibleWorkspaceIterMut},
    window::Window,
};

pub struct InputState {
    pub pointer_location: Point<f64, Logical>,
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

    pub shell_manager: ShellManager,

    pub dnd_icon: Arc<Mutex<Option<WlSurface>>>,
    pub cursor_status: Arc<Mutex<CursorImageStatus>>,

    pub input_state: InputState,

    pub seat_name: String,
    pub seat: Seat,
    pub session: AnodiumSession,

    pub start_time: std::time::Instant,
    last_update: Instant,

    pub config: ConfigVM,

    // Desktop Layout
    pub output_map: OutputMap,
    pub workspaces: HashMap<String, Box<dyn Positioner>>,
    pub active_workspace: Option<String>,
    pub grabed_window: Option<Window>,
    pub focused_window: Option<Window>,

    #[cfg(feature = "xwayland")]
    pub xwayland: XWayland<Self>,
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
        session: &AnodiumSession,
    ) -> (
        Seat,
        PointerHandle,
        KeyboardHandle,
        Arc<Mutex<CursorImageStatus>>,
    ) {
        let (mut seat, _) = Seat::new(
            &mut display.borrow_mut(),
            session.seat(),
            slog_scope::logger(),
        );

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
        display: Rc<RefCell<Display>>,
        handle: LoopHandle<'static, Self>,
        session: AnodiumSession,
    ) -> Self {
        let log = slog_scope::logger();

        // init the wayland connection
        Self::init_wayland_connection(&handle, &display);

        // Init the basic compositor globals

        init_shm_global(&mut display.borrow_mut(), vec![], log.clone());
        init_xdg_output_manager(&mut display.borrow_mut(), log.clone());

        let dnd_icon = Self::init_data_device(&display);

        let shell_manager =
            ShellManager::init_shell(&mut display.borrow_mut(), |event, mut ddata| {
                let state = ddata.get::<Anodium>().unwrap();
                state.on_shell_event(event);
            });

        let (seat, pointer, keyboard, cursor_status) = Self::init_seat(&display, &session);

        #[cfg(feature = "xwayland")]
        let xwayland = Self::init_xwayland_connection(&handle, &display);

        let event_sender = Self::init_config_channel(&handle);
        let output_map = OutputMap::new(event_sender.clone());

        let config = ConfigVM::new(event_sender, output_map.clone(), handle.clone()).unwrap();

        Self {
            handle,

            running: Arc::new(AtomicBool::new(true)),

            shell_manager,
            // desktop_layout: DesktopLayout::new(display.clone(), config.clone(), log.clone()),
            display,

            dnd_icon,
            cursor_status,

            input_state: InputState {
                pointer_location: (0.0, 0.0).into(),
                pointer,
                keyboard,
                modifiers_state: Default::default(),
                suppressed_keys: Vec::new(),
                pressed_keys: HashSet::new(),
            },

            seat_name: session.seat(),
            seat,
            session,

            start_time: Instant::now(),
            last_update: Instant::now(),

            config,

            output_map,
            workspaces: Default::default(),
            active_workspace: None,
            grabed_window: Default::default(),
            focused_window: Default::default(),

            #[cfg(feature = "xwayland")]
            xwayland,
        }
    }

    pub fn start(&mut self) {
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
}

impl Anodium {
    pub fn update(&mut self) {
        let elapsed = self.last_update.elapsed().as_secs_f64();

        self.shell_manager.refresh();

        for (_, w) in self.workspaces.iter_mut() {
            w.update(elapsed);
        }

        self.output_map.refresh();

        if let Some(focused_window) = &self.focused_window {
            if !focused_window.toplevel().alive() {
                self.update_focused_window(None);
            }
        }

        self.last_update = Instant::now();
    }

    pub fn render(
        &mut self,
        frame: &mut RenderFrame,
        output: &Output,
        pointer_image: Option<&Gles2Texture>,
    ) -> Result<(), smithay::backend::SwapBuffersError> {
        let (output_geometry, output_scale) = (output.geometry(), output.scale());

        frame.clear([0.1, 0.1, 0.1, 1.0])?;

        // Layers bellow windows
        self.draw_layers(frame, Layer::Background, output_geometry, output_scale)?;
        if let Some(wallaper) = output.get_wallpaper(frame.renderer) {
            frame.render_texture_at(
                &wallaper,
                Point::<i32, Logical>::from((0, 0))
                    .to_f64()
                    .to_physical(output_scale as f64)
                    .to_i32_round(),
                1,
                output_scale as f64,
                Transform::Normal,
                1.0,
            )?;
            //let src = Rectangle::from_loc_and_size((0, 0), (22, 35));
            //let dst = Rectangle::from_loc_and_size((1000, 400), (22, 35)).to_f64();

            /*frame.render_texture_from_to(
                &wallaper,
                output_geometry.to_buffer(1),
                dst,
                Transform::Normal,
                1.0,
            )?;*/
        }

        self.draw_layers(frame, Layer::Bottom, output_geometry, output_scale)?;

        // draw the windows
        self.draw_windows(frame, output_geometry, output_scale)?;

        // Layers above windows
        for layer in [Layer::Top, Layer::Overlay] {
            self.draw_layers(frame, layer, output_geometry, output_scale)?;
        }

        // Grab Debug:
        // if let Some(window) = self.desktop_layout.borrow().grabed_window.as_ref() {
        //     let loc: Point<i32, Logical> = window.location() + window.geometry().loc;
        //     let size: Size<i32, Logical> = window.geometry().size;
        //     let quad: Rectangle<i32, Logical> = Rectangle::from_loc_and_size(loc, size);

        //     if output_geometry.overlaps(quad) {
        //         frame.quad_pipeline.render(
        //             output_geometry.to_f64().to_physical(output_scale),
        //             quad.to_f64().to_physical(output_scale),
        //             frame.transform,
        //             &frame.context,
        //             0.1,
        //         );
        //     }
        // }

        {
            let space = output.active_workspace();
            let ui = &frame.imgui;

            imgui::Window::new("Workspace")
                .size([100.0, 20.0], imgui::Condition::Always)
                .position([0.0, 30.0], imgui::Condition::Always)
                .title_bar(false)
                .build(&ui, || {
                    ui.text(&format!("Workspace: {}", space));
                });
        }

        // Pointer Related:
        if output_geometry
            .to_f64()
            .contains(self.input_state.pointer_location)
        {
            let (ptr_x, ptr_y) = self.input_state.pointer_location.into();
            let relative_ptr_location =
                Point::<i32, Logical>::from((ptr_x as i32, ptr_y as i32)) - output_geometry.loc;
            // draw the dnd icon if applicable
            {
                let guard = self.dnd_icon.lock().unwrap();
                if let Some(ref wl_surface) = *guard {
                    if wl_surface.as_ref().is_alive() {
                        render::draw_dnd_icon(
                            frame,
                            wl_surface,
                            relative_ptr_location,
                            output_scale,
                        )?;
                    }
                }
            }

            // set cursor
            {
                let (ptr_x, ptr_y) = self.input_state.pointer_location.into();
                let relative_ptr_location =
                    Point::<i32, Logical>::from((ptr_x as i32, ptr_y as i32)) - output_geometry.loc;
                // draw the cursor as relevant
                {
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
                        render::draw_cursor(
                            frame,
                            wl_surface,
                            relative_ptr_location,
                            output_scale,
                        )?;
                    } else {
                        if let Some(pointer_image) = pointer_image {
                            frame.render_texture_at(
                                pointer_image,
                                relative_ptr_location
                                    .to_f64()
                                    .to_physical(output_scale as f64)
                                    .to_i32_round(),
                                1,
                                output_scale as f64,
                                Transform::Normal,
                                1.0,
                            )?;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

// Workspaces
impl Anodium {
    pub fn active_workspace(&mut self) -> &mut dyn Positioner {
        self.workspaces
            .get_mut(self.active_workspace.as_ref().unwrap())
            .unwrap()
            .as_mut()
    }

    pub fn visible_workspaces(&self) -> impl Iterator<Item = &dyn Positioner> {
        VisibleWorkspaceIter::new(&self.output_map, &self.workspaces)
    }

    pub fn visible_workspaces_mut(&mut self) -> impl Iterator<Item = &mut dyn Positioner> {
        VisibleWorkspaceIterMut::new(&self.output_map, &mut self.workspaces)
    }

    #[allow(dead_code)]
    pub fn find_workspace_by_surface<S: AsWlSurface>(
        &self,
        surface: &S,
    ) -> Option<&dyn Positioner> {
        for w in self.visible_workspaces() {
            if let Some(surface) = surface.as_surface() {
                if w.find_window(surface).is_some() {
                    return Some(w);
                }
            }
        }
        None
    }

    pub fn find_workspace_by_surface_mut<S: AsWlSurface>(
        &mut self,
        surface: &S,
    ) -> Option<&mut dyn Positioner> {
        for w in self.visible_workspaces_mut() {
            if let Some(surface) = surface.as_surface() {
                if w.find_window(surface).is_some() {
                    return Some(w);
                }
            }
        }
        None
    }

    pub fn update_workspaces_geometry(&mut self) {
        for output in self.output_map.iter() {
            let key = output.active_workspace();
            if let Some(w) = self.workspaces.get_mut(&key) {
                w.set_geometry(output.usable_geometry());
            }
        }
    }

    pub fn switch_workspace(&mut self, key: &str) {
        let already_active = self.output_map.iter().any(|o| &o.active_workspace() == key);

        if already_active {
            if let Some(workspace) = self.workspaces.get(key) {
                let geometry = workspace.geometry();
                let loc = geometry.loc;
                let size = geometry.size;

                self.input_state.pointer_location.x = (loc.x + size.w / 2) as f64;
                self.input_state.pointer_location.y = (loc.y + size.h / 2) as f64;
            }
        } else {
            for mut o in self.output_map.iter_mut() {
                if o.geometry()
                    .to_f64()
                    .contains(self.input_state.pointer_location)
                {
                    if self.workspaces.get(key).is_none() {
                        let positioner = Universal::new(Default::default(), Default::default());
                        self.workspaces.insert(key.into(), Box::new(positioner));
                    }
                    o.set_active_workspace(key.into());
                    break;
                }
            }

            self.active_workspace = Some(key.into());
            self.update_focused_window(None);
            self.clear_keyboard_focus();
            self.update_workspaces_geometry();
        }
    }

    pub fn update_focused_window(&mut self, window: Option<Window>) {
        self.config
            .anodize
            .windows
            .update_focused_window(window.clone());
        self.focused_window = window;
    }
}
