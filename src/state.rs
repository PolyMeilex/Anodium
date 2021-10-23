use std::{
    cell::RefCell,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Instant,
};

use smithay::{
    backend::renderer::Frame,
    reexports::{
        calloop::{generic::Generic, Interest, LoopHandle, Mode, PostAction},
        wayland_server::{protocol::wl_surface::WlSurface, Display},
    },
    utils::{Logical, Point, Rectangle, Size},
    wayland::{
        data_device::{default_action_chooser, init_data_device, set_data_device_focus, DataDeviceEvent},
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
    backend::Backend,
    config::ConfigVM,
    desktop_layout::{DesktopLayout, Output},
    render::{self, renderer::RenderFrame},
    shell::init_shell,
    shell::not_mapped_list::NotMappedList,
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

    pub not_mapped_list: Rc<RefCell<NotMappedList>>,

    pub desktop_layout: Rc<RefCell<DesktopLayout>>,

    pub dnd_icon: Arc<Mutex<Option<WlSurface>>>,

    pub input_state: InputState,

    pub seat_name: String,
    pub seat: Seat,

    pub start_time: std::time::Instant,
    pub fps: fps_ticker::Fps,
    last_update: Instant,

    pub config: ConfigVM,
    pub log: slog::Logger,
}

impl Anodium {
    pub fn update(&mut self) {
        let elapsed = self.last_update.elapsed().as_secs_f64();

        // anodium.maximize_animation.update(elapsed);

        self.desktop_layout.borrow_mut().update(elapsed);

        self.last_update = Instant::now();
        self.fps.tick();
    }

    pub fn render(
        &mut self,
        frame: &mut RenderFrame,
        (output_geometry, output_scale): (Rectangle<i32, Logical>, f64),
    ) -> Result<(), smithay::backend::SwapBuffersError> {
        frame.clear([0.1, 0.1, 0.1, 1.0])?;

        // Layers bellow windows
        for layer in [Layer::Background, Layer::Bottom] {
            self.draw_layers(frame, layer, output_geometry, output_scale, &self.log)?;
        }

        // draw the windows
        self.draw_windows(frame, output_geometry, output_scale, &self.log)?;

        // Layers above windows
        for layer in [Layer::Top, Layer::Overlay] {
            self.draw_layers(frame, layer, output_geometry, output_scale, &self.log)?;
        }

        // Grab Debug:
        if let Some(window) = self.desktop_layout.borrow().grabed_window.as_ref() {
            let loc: Point<i32, Logical> = window.location() + window.geometry().loc;
            let size: Size<i32, Logical> = window.geometry().size;
            let quad: Rectangle<i32, Logical> = Rectangle::from_loc_and_size(loc, size);

            if output_geometry.overlaps(quad) {
                frame.quad_pipeline.render(
                    output_geometry.to_f64().to_physical(output_scale),
                    quad.to_f64().to_physical(output_scale),
                    frame.transform,
                    &frame.context,
                    0.1,
                );
            }
        }

        #[cfg(feature = "debug")]
        if let Some(o) = self
            .desktop_layout
            .borrow()
            .output_map
            .find_by_position(output_geometry.loc)
        {
            let space = o.active_workspace();
            let ui = &frame.imgui_frame;

            imgui::Window::new(imgui::im_str!("Workspace"))
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
                            &self.log,
                        )?;
                    }
                }
            }
        }

        Ok(())
    }

    pub fn add_output<N, CB>(
        &mut self,
        name: N,
        physical: PhysicalProperties,
        mode: smithay::wayland::output::Mode,
        after: CB,
    ) where
        N: AsRef<str>,
        CB: FnOnce(&Output),
    {
        self.desktop_layout
            .borrow_mut()
            .add_output(name, physical, mode, after);
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

pub struct BackendState<BackendData> {
    pub handle: LoopHandle<'static, Self>,
    pub backend_data: BackendData,
    pub cursor_status: Arc<Mutex<CursorImageStatus>>,

    pub anodium: Anodium,

    #[cfg(feature = "xwayland")]
    pub xwayland: XWayland<Self>,

    pub log: slog::Logger,
}

impl<BackendData: Backend + 'static> BackendState<BackendData> {
    pub fn init(
        display: Rc<RefCell<Display>>,
        handle: LoopHandle<'static, Self>,
        backend_data: BackendData,
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
                            error!(state.anodium.log, "I/O error on the Wayland display: {}", e);
                            state.anodium.running.store(false, Ordering::SeqCst);
                            Err(e)
                        }
                    }
                },
            )
            .expect("Failed to init the wayland event source.");

        // Init the basic compositor globals

        init_shm_global(&mut display.borrow_mut(), vec![], log.clone());

        init_shell::<BackendData>(display.clone(), log.clone());

        init_xdg_output_manager(&mut display.borrow_mut(), log.clone());

        let socket_name = display
            .borrow_mut()
            .add_socket_auto()
            .unwrap()
            .into_string()
            .unwrap();

        info!(log, "Listening on wayland socket"; "name" => socket_name.clone());
        ::std::env::set_var("WAYLAND_DISPLAY", &socket_name);

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
        let seat_name = backend_data.seat_name();

        let (mut seat, _) = Seat::new(&mut display.borrow_mut(), seat_name.clone(), log.clone());

        let cursor_status = Arc::new(Mutex::new(CursorImageStatus::Default));

        let cursor_status2 = cursor_status.clone();
        let pointer = seat.add_pointer(move |new_status| {
            // TODO: hide winit system cursor when relevant
            *cursor_status2.lock().unwrap() = new_status
        });

        init_tablet_manager_global(&mut display.borrow_mut());

        let cursor_status3 = cursor_status.clone();
        seat.tablet_seat().on_cursor_surface(move |_tool, new_status| {
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
            let ret = handle.insert_source(channel, |event, _, state| match event {
                XWaylandEvent::Ready { connection, client } => state.xwayland_ready(connection, client),
                XWaylandEvent::Exited => state.xwayland_exited(),
            });
            if let Err(e) = ret {
                error!(
                    log,
                    "Failed to insert the XWaylandSource into the event loop: {}", e
                );
            }
            xwayland
        };

        crate::wayland::decoration::init_decoration_manager(&mut display.borrow_mut());

        let config = ConfigVM::new().unwrap();

        BackendState {
            handle,
            backend_data,
            cursor_status,
            anodium: Anodium {
                running: Arc::new(AtomicBool::new(true)),
                desktop_layout: Rc::new(RefCell::new(DesktopLayout::new(
                    display.clone(),
                    config.clone(),
                    log.clone(),
                ))),

                display,
                not_mapped_list: Default::default(),

                dnd_icon,

                input_state: InputState {
                    pointer_location: (0.0, 0.0).into(),
                    pointer: pointer,
                    keyboard: keyboard,
                    modifiers_state: Default::default(),
                    suppressed_keys: Vec::new(),
                },

                seat_name,
                seat,

                start_time: Instant::now(),
                fps: fps_ticker::Fps::default(),
                last_update: Instant::now(),

                config,
                log: log.clone(),
            },
            log,
            #[cfg(feature = "xwayland")]
            xwayland,
        }
    }
}
