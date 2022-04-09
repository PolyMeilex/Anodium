use std::{cell::RefCell, collections::HashSet, rc::Rc, time::Instant};

use anodium_framework::pointer_icon::PointerIcon;
use anodium_protocol::server::AnodiumProtocol;
use calloop::{
    channel::{self, Channel},
    LoopSignal,
};
use smithay::{
    backend::renderer::gles2::{Gles2Renderer, Gles2Texture},
    desktop::{self, space::SurfaceTree},
    reexports::{
        calloop::{self, channel::Sender, generic::Generic, Interest, LoopHandle, PostAction},
        wayland_server::Display,
    },
    utils::{Logical, Point, Rectangle},
    wayland::{
        data_device,
        output::xdg::init_xdg_output_manager,
        seat::{KeyboardHandle, ModifiersState, PointerHandle, Seat, XkbConfig},
        shm::init_shm_global,
    },
};

#[cfg(feature = "xwayland")]
use smithay::xwayland::{XWayland, XWaylandEvent};

use anodium_backend::{utils::cursor::PointerElement, BackendRequest};
use smithay_egui::EguiFrame;

use crate::{
    cli::AnodiumOptions,
    config::{eventloop::ConfigEvent, ConfigVM},
    framework::shell::ShellManager,
    output_manager::{Output, OutputManager},
    region_manager::RegionManager,
};

smithay::custom_elements! {
    pub CustomElem<=Gles2Renderer>;
    SurfaceTree=SurfaceTree,
    PointerElement=PointerElement,
    EguiFrame=EguiFrame,
}

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
    pub loop_signal: LoopSignal,

    pub display: Rc<RefCell<Display>>,

    pub shell_manager: ShellManager<Self>,

    pub pointer_icon: PointerIcon,

    pub input_state: Rc<RefCell<InputState>>,

    pub seat: Seat,

    pub options: AnodiumOptions,

    pub start_time: std::time::Instant,
    last_update: Instant,

    pub config: ConfigVM,

    // Desktop
    pub anodium_protocol: AnodiumProtocol,
    pub output_manager: OutputManager,
    pub region_manager: RegionManager,
    //pub workspace: Workspace,
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
                            state.loop_signal.stop();
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
    fn init_data_device(display: &Rc<RefCell<Display>>, pointer_icon: PointerIcon) {
        data_device::init_data_device(
            &mut display.borrow_mut(),
            move |event| pointer_icon.on_data_device_event(event),
            data_device::default_action_chooser,
            slog_scope::logger(),
        );
    }

    /// init wayland seat, keyboard and pointer
    fn init_seat(
        display: &Rc<RefCell<Display>>,
        seat_name: String,
        pointer_icon: PointerIcon,
    ) -> (Seat, PointerHandle, KeyboardHandle) {
        let (mut seat, _) = Seat::new(&mut display.borrow_mut(), seat_name, slog_scope::logger());

        let pointer = seat.add_pointer(move |status| pointer_icon.on_new_cursor(status));

        let keyboard = seat
            .add_keyboard(XkbConfig::default(), 200, 25, |seat, focus| {
                data_device::set_data_device_focus(seat, focus.and_then(|s| s.as_ref().client()))
            })
            .expect("Failed to initialize the keyboard");

        (seat, pointer, keyboard)
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
        loop_signal: LoopSignal,
        display: Rc<RefCell<Display>>,
        seat_name: String,
        options: AnodiumOptions,
    ) -> (Self, Channel<BackendRequest>) {
        let log = slog_scope::logger();

        let (backend_tx, backend_rx) = channel::channel();

        // init the wayland connection
        Self::init_wayland_connection(&handle, &display);

        // Init the basic compositor globals

        init_shm_global(&mut display.borrow_mut(), vec![], log.clone());
        init_xdg_output_manager(&mut display.borrow_mut(), log.clone());

        let pointer_icon = PointerIcon::new();
        Self::init_data_device(&display, pointer_icon.clone());

        let shell_manager = ShellManager::init_shell(&mut display.borrow_mut());

        let (seat, pointer, keyboard) = Self::init_seat(&display, seat_name, pointer_icon.clone());

        let (anodium_protocol, _global) = AnodiumProtocol::init(&mut display.borrow_mut());

        #[cfg(feature = "xwayland")]
        let xwayland = Self::init_xwayland_connection(&handle, &display);

        let config_tx = Self::init_config_channel(&handle);
        let output_map = OutputManager::new();
        let region_map = RegionManager::new();
        let input_state = Rc::new(RefCell::new(InputState {
            pointer_location: (0.0, 0.0).into(),
            previous_pointer_location: (0.0, 0.0).into(),
            pointer,
            keyboard,
            modifiers_state: Default::default(),
            suppressed_keys: Vec::new(),
            pressed_keys: HashSet::new(),
        }));

        let config = ConfigVM::new(
            config_tx.clone(),
            output_map.clone(),
            region_map.clone(),
            handle.clone(),
            input_state.clone(),
            options.config.clone(),
        )
        .unwrap();

        (
            Self {
                loop_signal,

                shell_manager,
                display,

                pointer_icon,

                input_state,

                seat,

                options,

                start_time: Instant::now(),
                last_update: Instant::now(),

                config,

                anodium_protocol,
                output_manager: output_map,
                region_manager: region_map,
                //workspace: Workspace::new(),
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
        self.region_manager.refresh();

        if let Some(focused_window) = &self.focused_window {
            if !focused_window.toplevel().alive() {
                self.update_focused_window(None);
            }
        }

        self.last_update = Instant::now();
    }

    pub fn render(
        &mut self,
        renderer: &mut Gles2Renderer,
        output: &Output,
        age: usize,
        pointer_image: Option<&Gles2Texture>,
    ) -> Result<Option<Vec<Rectangle<i32, Logical>>>, smithay::backend::SwapBuffersError> {
        let region = if let Some(region) = self.region_manager.find_output_region(output) {
            region
        } else {
            return Ok(None);
        };

        let workspace = region.active_workspace();
        let output_geometry = workspace.space().output_geometry(output).unwrap();

        let mut elems: Vec<CustomElem> = Vec::new();
        {
            let input_state = self.input_state.borrow();
            let frame = output.render_egui_shell(
                &self.start_time,
                &input_state.modifiers_state,
                &self.config_tx,
            );
            elems.push(frame.into());
        }
        let mut input_state = self.input_state.borrow_mut();
        // Pointer Related:
        if region.contains(input_state.pointer_location) {
            let (ptr_x, ptr_y) = input_state.pointer_location.into();
            let relative_location = Point::<i32, Logical>::from((ptr_x as i32, ptr_y as i32))
                - output_geometry.loc
                - region.position();

            if let Some(tree) = self.pointer_icon.prepare_dnd_icon(relative_location) {
                elems.push(tree.into());
            }

            if let Some(tree) = self.pointer_icon.prepare_cursor_icon(relative_location) {
                elems.push(tree.into());
            } else if let Some(texture) = pointer_image {
                elems.push(
                    PointerElement::new(
                        texture.clone(),
                        relative_location,
                        input_state.pointer_location != input_state.previous_pointer_location,
                    )
                    .into(),
                );
            }

            input_state.previous_pointer_location = input_state.pointer_location;
        }

        let render_result = workspace
            .space_mut()
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
        info!("update focused window: {:?}", window);
        self.region_manager.iter().for_each(|r| {
            r.for_each_workspace(|w| {
                w.space().windows().for_each(|w| {
                    w.set_activated(false);
                });
            });
        });

        if let Some(window) = window {
            window.set_activated(true);
        }

        self.region_manager.iter().for_each(|r| {
            r.for_each_workspace(|w| {
                w.space().windows().for_each(|w| w.configure());
            });
            if let Some(window) = window {
                if let Some(workspace) = r.find_window_workspace(window) {
                    workspace.space_mut().raise_window(window, true);
                }
            }
        });

        self.focused_window = window.cloned();
    }
}
