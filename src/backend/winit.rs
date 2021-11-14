use std::{cell::RefCell, rc::Rc, sync::atomic::Ordering, time::Duration};

use smithay::backend::winit::WinitEvent;
use smithay::{
    backend::renderer::{ImportDma, ImportEgl, Transform},
    reexports::calloop::timer::Timer,
    wayland::dmabuf::init_dmabuf_global,
};
use smithay::{
    backend::winit,
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

use super::session::AnodiumSession;

use crate::{render::renderer::RenderFrame, render::*, state::BackendState};

pub const OUTPUT_NAME: &str = "winit";

mod input;

#[derive(Default)]
pub struct WinitData {}

pub fn run_winit(
    display: Rc<RefCell<Display>>,
    event_loop: &mut EventLoop<'static, BackendState>,
    log: Logger,
) -> Result<BackendState, ()> {
    let (renderer, mut input) = winit::init(log.clone()).map_err(|err| {
        slog::crit!(log, "Failed to initialize Winit backend: {}", err);
    })?;
    let renderer = Rc::new(RefCell::new(renderer));

    if renderer
        .borrow_mut()
        .renderer()
        .bind_wl_display(&display.borrow())
        .is_ok()
    {
        info!("EGL hardware-acceleration enabled");
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
            move |buffer, _| {
                renderer
                    .borrow_mut()
                    .renderer()
                    .import_dmabuf(buffer)
                    .is_ok()
            },
            log.clone(),
        );
    };

    let size = renderer.borrow().window_size().physical_size;

    /*
     * Initialize the globals
     */

    let mut state = BackendState::init(
        display.clone(),
        event_loop.handle(),
        AnodiumSession::new_winit(),
        log.clone(),
    );

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

    let mut imgui = imgui::Context::create();
    {
        imgui.set_ini_filename(None);
        let io = imgui.io_mut();
        io.display_framebuffer_scale = [1.0 as f32, 1.0 as f32];
        io.display_size = [size.w as f32, size.h as f32];
    }

    let imgui_pipeline = renderer
        .borrow_mut()
        .renderer()
        .with_context(|_, gles| imgui_smithay_renderer::Renderer::new(gles, &mut imgui))
        .unwrap();

    info!("Initialization completed, starting the main loop.");

    let timer = Timer::new().unwrap();
    let timer_handle = timer.handle();

    let fps = fps_ticker::Fps::default();

    event_loop
        .handle()
        .insert_source(timer, move |_: (), handle, state| {
            match input.dispatch_new_events(|event| {
                if let WinitEvent::Resized { size, scale_factor } = &event {
                    let io = imgui.io_mut();
                    io.display_framebuffer_scale = [*scale_factor as f32, *scale_factor as f32];
                    io.display_size = [size.w as f32, size.h as f32];
                }

                state.process_winit_event(event)
            }) {
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
                            .render(|renderer, frame| {
                                let mut frame = RenderFrame {
                                    transform: Transform::Normal,
                                    renderer,
                                    frame,
                                };

                                state
                                    .anodium
                                    .render(&mut frame, (output_geometry, output_scale))
                                    .unwrap();

                                {
                                    let ui = imgui.frame();
                                    draw_fps(&ui, 1.0, fps.avg());
                                    let draw_data = ui.render();
                                    frame
                                        .renderer
                                        .with_context(|_renderer, gles| {
                                            imgui_pipeline.render(
                                                Transform::Normal,
                                                gles,
                                                &draw_data,
                                            );
                                        })
                                        .unwrap();
                                }

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
                                            &mut frame,
                                            surface,
                                            (x as i32, y as i32).into(),
                                            output_scale,
                                        )
                                        .unwrap();
                                    }
                                }
                            })
                            .unwrap();
                    }

                    let time = start_time.elapsed().as_millis() as u32;
                    state.anodium.send_frames(time);

                    fps.tick();

                    handle.add_timeout(Duration::from_millis(16), ());
                }
                Err(winit::WinitError::WindowClosed) => {
                    state.anodium.running.store(false, Ordering::SeqCst);
                }
            }
        })
        .unwrap();
    timer_handle.add_timeout(Duration::ZERO, ());

    Ok(state)
}
