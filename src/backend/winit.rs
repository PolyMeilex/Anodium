use std::{cell::RefCell, rc::Rc, sync::atomic::Ordering, time::Duration};

use smithay::{
    backend::renderer::{ImportDma, ImportEgl},
    reexports::calloop::timer::Timer,
    wayland::dmabuf::init_dmabuf_global,
};
use smithay::{
    backend::{input::InputBackend, winit},
    reexports::{
        calloop::EventLoop,
        wayland_server::{protocol::wl_output, Display},
    },
    wayland::{
        output::{Mode, PhysicalProperties},
        seat::CursorImageStatus,
    },
};

use slog::Logger;

use super::Backend;
use crate::{render::AnodiumRenderer, render::*, state::BackendState};

pub const OUTPUT_NAME: &str = "winit";

mod input;

pub struct WinitData {}

impl Backend for WinitData {
    fn seat_name(&self) -> String {
        String::from("winit")
    }

    fn change_vt(&mut self, _vt: i32) {}
}

pub fn run_winit(
    display: Rc<RefCell<Display>>,
    event_loop: &mut EventLoop<'static, BackendState<WinitData>>,
    log: Logger,
) -> Result<BackendState<WinitData>, ()> {
    let (renderer, mut input) = winit::init(log.clone()).map_err(|err| {
        slog::crit!(log, "Failed to initialize Winit backend: {}", err);
    })?;
    let renderer = AnodiumRenderer::new(renderer);
    let renderer = Rc::new(RefCell::new(renderer));

    if renderer
        .borrow_mut()
        .renderer()
        .bind_wl_display(&display.borrow())
        .is_ok()
    {
        info!(log, "EGL hardware-acceleration enabled");
        let dmabuf_formats = renderer
            .borrow_mut()
            .renderer()
            .dmabuf_formats()
            .cloned()
            .collect::<Vec<_>>();
        let renderer = renderer.clone();
        init_dmabuf_global(
            &mut *display.borrow_mut(),
            dmabuf_formats,
            move |buffer, _| renderer.borrow_mut().renderer().import_dmabuf(buffer).is_ok(),
            log.clone(),
        );
    };

    let size = renderer.borrow().window_size().physical_size;

    /*
     * Initialize the globals
     */

    let data = WinitData {};
    let mut state = BackendState::init(display.clone(), event_loop.handle(), data, log.clone());

    let mode = Mode {
        size,
        refresh: 60_000,
    };

    state.anodium.add_output(
        OUTPUT_NAME,
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: wl_output::Subpixel::Unknown,
            make: "Smithay".into(),
            model: "Winit".into(),
        },
        mode,
        |_| {},
    );

    let start_time = std::time::Instant::now();

    #[cfg(feature = "xwayland")]
    state.start_xwayland();

    info!(log, "Initialization completed, starting the main loop.");

    let timer = Timer::new().unwrap();
    let timer_handle = timer.handle();

    {
        event_loop
            .handle()
            .insert_source(timer, move |_: (), handle, state| {
                match input.dispatch_new_events(|event| state.process_input_event(event)) {
                    Ok(()) => {
                        let mut renderer = renderer.borrow_mut();
                        let outputs: Vec<_> = state
                            .anodium
                            .desktop_layout
                            .borrow()
                            .output_map
                            .iter()
                            .map(|o| (o.geometry(), o.scale()))
                            .collect();

                        for (output_geometry, output_scale) in outputs {
                            renderer
                                .render_winit(|frame| {
                                    state
                                        .anodium
                                        .render(frame, (output_geometry, output_scale))
                                        .unwrap();

                                    // draw the cursor as relevant
                                    {
                                        let (x, y) = state.anodium.input_state.pointer_location.into();
                                        let mut guard = state.cursor_status.lock().unwrap();
                                        // reset the cursor if the surface is no longer alive
                                        let mut reset = false;
                                        if let CursorImageStatus::Image(ref surface) = *guard {
                                            reset = !surface.as_ref().is_alive();
                                        }
                                        if reset {
                                            *guard = CursorImageStatus::Default;
                                        }

                                        // draw as relevant
                                        if let CursorImageStatus::Image(ref surface) = *guard {
                                            draw_cursor(
                                                frame,
                                                surface,
                                                (x as i32, y as i32).into(),
                                                output_scale,
                                                &log,
                                            )
                                            .unwrap();
                                        }
                                    }

                                    let fps = state.anodium.fps.avg().round() as u32;

                                    #[cfg(feature = "debug")]
                                    {
                                        draw_fps(frame, output_scale as f64, fps).unwrap();
                                    }
                                    #[cfg(not(feature = "debug"))]
                                    let _ = fps;

                                    //
                                })
                                .unwrap();
                        }

                        let time = start_time.elapsed().as_millis() as u32;
                        state.anodium.send_frames(time);

                        state.anodium.display.borrow_mut().flush_clients(&mut ());
                        state.anodium.update();

                        handle.add_timeout(Duration::from_millis(16), ());
                    }
                    Err(winit::WinitInputError::WindowClosed) => {
                        state.anodium.running.store(false, Ordering::SeqCst);
                    }
                }
            })
            .unwrap();
        timer_handle.add_timeout(Duration::ZERO, ());
    }

    Ok(state)
}
