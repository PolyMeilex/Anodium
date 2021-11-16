use std::{
    cell::RefCell,
    collections::HashMap,
    path::PathBuf,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

use smithay::{
    backend::{renderer::Frame, session::Session},
    nix::libc::dev_t,
    reexports::{
        calloop::{generic::Generic, Interest, LoopHandle, Mode, PostAction},
        wayland_server::{protocol::wl_surface::WlSurface, Display},
    },
    utils::{Logical, Point, Rectangle},
    wayland::{
        data_device::{
            default_action_chooser, init_data_device, set_data_device_focus, DataDeviceEvent,
        },
        output::{xdg::init_xdg_output_manager, PhysicalProperties},
        seat::{CursorImageStatus, KeyboardHandle, ModifiersState, PointerHandle, Seat, XkbConfig},
        shell::wlr_layer::Layer,
        shm::init_shm_global,
        tablet_manager::{init_tablet_manager_global, TabletSeatTrait},
    },
};

#[cfg(feature = "xwayland")]
use smithay::xwayland::{XWayland, XWaylandEvent};

use crate::{
    backend::{session::AnodiumSession, udev},
    config::ConfigVM,
    desktop_layout::{DesktopLayout, Output},
    render::{self, renderer::RenderFrame},
    shell::{
        move_surface_grab::MoveSurfaceGrab,
        shell_manager::{ShellEvent, ShellManager},
    },
};

pub struct InputState {
    pub pointer_location: Point<f64, Logical>,
    pub pointer: PointerHandle,

    pub keyboard: KeyboardHandle,
    pub modifiers_state: ModifiersState,

    pub suppressed_keys: Vec<u32>,
}

pub struct Anodium {
    pub running: Arc<AtomicBool>,
    pub display: Rc<RefCell<Display>>,

    pub shell_manager: ShellManager,
    pub desktop_layout: Rc<RefCell<DesktopLayout>>,

    pub dnd_icon: Arc<Mutex<Option<WlSurface>>>,

    pub input_state: InputState,

    pub seat_name: String,
    pub seat: Seat,
    pub session: AnodiumSession,

    pub start_time: std::time::Instant,
    last_update: Instant,

    pub config: ConfigVM,
    pub log: slog::Logger,
}

impl Anodium {
    pub fn update(&mut self) {
        let elapsed = self.last_update.elapsed().as_secs_f64();

        self.shell_manager.refresh();

        self.desktop_layout.borrow_mut().update(elapsed);

        self.last_update = Instant::now();
    }

    pub fn render(
        &mut self,
        frame: &mut RenderFrame,
        (output_geometry, output_scale): (Rectangle<i32, Logical>, f64),
    ) -> Result<(), smithay::backend::SwapBuffersError> {
        frame.clear([0.1, 0.1, 0.1, 1.0])?;

        // Layers bellow windows
        for layer in [Layer::Background, Layer::Bottom] {
            self.draw_layers(frame, layer, output_geometry, output_scale)?;
        }

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

        // #[cfg(feature = "debug")]
        // if let Some(o) = self
        //     .desktop_layout
        //     .borrow()
        //     .output_map
        //     .find_by_position(output_geometry.loc)
        // {
        //     let space = o.active_workspace();
        //     let ui = &frame.imgui_frame;

        //     imgui::Window::new(imgui::im_str!("Workspace"))
        //         .size([100.0, 20.0], imgui::Condition::Always)
        //         .position([0.0, 30.0], imgui::Condition::Always)
        //         .title_bar(false)
        //         .build(&ui, || {
        //             ui.text(&format!("Workspace: {}", space));
        //         });
        // }

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
        }

        Ok(())
    }

    fn on_shell_event(&mut self, event: ShellEvent) {
        //
        match event {
            ShellEvent::WindowCreated { window } => {
                let mut space = self.desktop_layout.borrow_mut();
                space.active_workspace().map_toplevel(window, true);
            }

            ShellEvent::WindowMove {
                toplevel,
                start_data,
                seat,
                serial,
            } => {
                let mut desktop_layout = self.desktop_layout.borrow_mut();
                let pointer = seat.get_pointer().unwrap();

                if let Some(space) = desktop_layout.find_workspace_by_surface_mut(&toplevel) {
                    if let Some(res) = space.move_request(&toplevel, &seat, serial, &start_data) {
                        if let Some(window) = space.unmap_toplevel(&toplevel) {
                            desktop_layout.grabed_window = Some(window);

                            let grab = MoveSurfaceGrab {
                                start_data,
                                toplevel,
                                initial_window_location: res.initial_window_location,
                                desktop_layout: self.desktop_layout.clone(),
                            };
                            pointer.set_grab(grab, serial);
                        }
                    }
                }
            }
            ShellEvent::WindowResize {
                toplevel,
                start_data,
                seat,
                edges,
                serial,
            } => {
                if let Some(space) = self
                    .desktop_layout
                    .borrow_mut()
                    .find_workspace_by_surface_mut(&toplevel)
                {
                    space.resize_request(&toplevel, &seat, serial, start_data, edges);
                }
            }

            ShellEvent::WindowMaximize { toplevel } => {
                if let Some(space) = self
                    .desktop_layout
                    .borrow_mut()
                    .find_workspace_by_surface_mut(&toplevel)
                {
                    space.maximize_request(&toplevel);
                }
            }
            ShellEvent::WindowUnMaximize { toplevel } => {
                if let Some(space) = self
                    .desktop_layout
                    .borrow_mut()
                    .find_workspace_by_surface_mut(&toplevel)
                {
                    space.unmaximize_request(&toplevel);
                }
            }

            ShellEvent::LayerCreated {
                surface, output, ..
            } => {
                self.desktop_layout
                    .borrow_mut()
                    .insert_layer(output, surface);
            }
            ShellEvent::LayerAckConfigure { .. } => {
                self.desktop_layout.borrow_mut().arrange_layers();
            }

            ShellEvent::SurfaceCommit { surface } => {
                let found = self
                    .desktop_layout
                    .borrow()
                    .output_map
                    .iter()
                    .any(|o| o.layer_map().find(&surface).is_some());

                if found {
                    self.desktop_layout.borrow_mut().arrange_layers();
                }
            }
            _ => {}
        }
    }

    pub fn add_output<CB>(&mut self, output: Output, after: CB)
    where
        CB: FnOnce(&Output),
    {
        self.desktop_layout.borrow_mut().add_output(output, after);
    }

    pub fn retain_outputs<F>(&mut self, f: F)
    where
        F: FnMut(&Output) -> bool,
    {
        self.desktop_layout.borrow_mut().retain_outputs(f);
    }

    pub fn send_frames(&self, time: u32) {
        self.desktop_layout.borrow().send_frames(time);
    }
}

pub struct BackendState {
    pub handle: LoopHandle<'static, Self>,
    pub cursor_status: Arc<Mutex<CursorImageStatus>>,

    pub anodium: Anodium,

    #[cfg(feature = "xwayland")]
    pub xwayland: XWayland<Self>,

    // Backend
    pub primary_gpu: Option<PathBuf>,
    pub udev_devices: HashMap<dev_t, udev::UdevDeviceData>,
    pub pointer_image: crate::cursor::Cursor,

    pub log: slog::Logger,
}

impl BackendState {
    pub fn init(
        display: Rc<RefCell<Display>>,
        handle: LoopHandle<'static, Self>,
        session: AnodiumSession,
        log: slog::Logger,
    ) -> Self {
        // init the wayland connection
        handle
            .insert_source(
                Generic::from_fd(display.borrow().get_poll_fd(), Interest::READ, Mode::Level),
                move |_, _, state: &mut Self| {
                    let display = state.anodium.display.clone();
                    let mut display = display.borrow_mut();
                    match display.dispatch(std::time::Duration::from_millis(0), state) {
                        Ok(_) => Ok(PostAction::Continue),
                        Err(e) => {
                            error!("I/O error on the Wayland display: {}", e);
                            state.anodium.running.store(false, Ordering::SeqCst);
                            Err(e)
                        }
                    }
                },
            )
            .expect("Failed to init the wayland event source.");

        // Init the basic compositor globals

        init_shm_global(&mut display.borrow_mut(), vec![], log.clone());

        let shell_manager =
            ShellManager::init_shell(&mut display.borrow_mut(), |event, mut ddata| {
                let state = ddata.get::<BackendState>().unwrap();
                state.anodium.on_shell_event(event);
            });

        // init_shell(display.clone(), log.clone());

        init_xdg_output_manager(&mut display.borrow_mut(), log.clone());

        // init data device

        let dnd_icon = Arc::new(Mutex::new(None));

        let dnd_icon2 = dnd_icon.clone();
        init_data_device(
            &mut display.borrow_mut(),
            move |event| match event {
                DataDeviceEvent::DnDStarted { icon, .. } => {
                    *dnd_icon2.lock().unwrap() = icon;
                }
                DataDeviceEvent::DnDDropped => {
                    *dnd_icon2.lock().unwrap() = None;
                }
                _ => {}
            },
            default_action_chooser,
            log.clone(),
        );

        // init input
        let seat_name = session.seat();

        let (mut seat, _) = Seat::new(&mut display.borrow_mut(), seat_name.clone(), log.clone());

        let cursor_status = Arc::new(Mutex::new(CursorImageStatus::Default));

        let cursor_status2 = cursor_status.clone();
        let pointer = seat.add_pointer(move |new_status| {
            // TODO: hide winit system cursor when relevant
            *cursor_status2.lock().unwrap() = new_status
        });

        init_tablet_manager_global(&mut display.borrow_mut());

        let cursor_status3 = cursor_status.clone();
        seat.tablet_seat()
            .on_cursor_surface(move |_tool, new_status| {
                // TODO: tablet tools should have their own cursors
                *cursor_status3.lock().unwrap() = new_status;
            });

        let keyboard = seat
            .add_keyboard(XkbConfig::default(), 200, 25, |seat, focus| {
                set_data_device_focus(seat, focus.and_then(|s| s.as_ref().client()))
            })
            .expect("Failed to initialize the keyboard");

        #[cfg(feature = "xwayland")]
        let xwayland = {
            let (xwayland, channel) = XWayland::new(handle.clone(), display.clone(), log.clone());

            let ret = handle.insert_source(channel, {
                let handle = handle.clone();
                move |event, _, state| match event {
                    XWaylandEvent::Ready { connection, client } => state
                        .anodium
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
        };

        let config = ConfigVM::new().unwrap();

        BackendState {
            handle,
            cursor_status,
            anodium: Anodium {
                running: Arc::new(AtomicBool::new(true)),

                shell_manager,
                desktop_layout: Rc::new(RefCell::new(DesktopLayout::new(
                    display.clone(),
                    config.clone(),
                    log.clone(),
                ))),

                display,
                dnd_icon,

                input_state: InputState {
                    pointer_location: (0.0, 0.0).into(),
                    pointer,
                    keyboard,
                    modifiers_state: Default::default(),
                    suppressed_keys: Vec::new(),
                },

                seat_name,
                seat,
                session,

                start_time: Instant::now(),
                last_update: Instant::now(),

                config,
                log: log.clone(),
            },
            log: log.clone(),
            #[cfg(feature = "xwayland")]
            xwayland,

            primary_gpu: None,
            udev_devices: Default::default(),
            pointer_image: crate::cursor::Cursor::load(&log),
        }
    }

    pub fn start(&mut self) {
        let socket_name = self
            .anodium
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
