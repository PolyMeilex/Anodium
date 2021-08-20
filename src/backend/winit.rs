use std::{cell::RefCell, rc::Rc, sync::atomic::Ordering};

#[cfg(feature = "debug")]
use smithay::backend::renderer::gles2::Gles2Texture;
use smithay::{
    backend::renderer::{ImportDma, ImportEgl},
    wayland::dmabuf::init_dmabuf_global,
};
use smithay::{
    backend::{input::InputBackend, winit, SwapBuffersError},
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

pub struct WinitData {
    #[cfg(feature = "debug")]
    fps_texture: Gles2Texture,
    #[cfg(feature = "debug")]
    pub fps: fps_ticker::Fps,
}

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
) -> Result<(), ()> {
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

    let data = WinitData {
        #[cfg(feature = "debug")]
        fps_texture: import_bitmap(
            &mut renderer.borrow_mut().renderer(),
            &image::io::Reader::with_format(std::io::Cursor::new(FPS_NUMBERS_PNG), image::ImageFormat::Png)
                .decode()
                .unwrap()
                .to_rgba8(),
        )
        .expect("Unable to upload FPS texture"),
        #[cfg(feature = "debug")]
        fps: fps_ticker::Fps::default(),
    };
    let mut state = BackendState::init(display.clone(), event_loop.handle(), data, log.clone());

    let mode = Mode {
        size,
        refresh: 60_000,
    };

    state.main_state.add_output(
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
    let mut cursor_visible = true;

    #[cfg(feature = "xwayland")]
    state.start_xwayland();

    info!(log, "Initialization completed, starting the main loop.");

    while state.main_state.running.load(Ordering::SeqCst) {
        if input
            .dispatch_new_events(|event| state.process_input_event(event))
            .is_err()
        {
            state.main_state.running.store(false, Ordering::SeqCst);
            break;
        }

        // drawing logic
        {
            let mut renderer = renderer.borrow_mut();
            // This is safe to do as with winit we are guaranteed to have exactly one output
            let (output_geometry, output_scale) = state
                .main_state
                .desktop_layout
                .output_map
                .find_by_name(OUTPUT_NAME)
                .map(|output| (output.geometry(), output.scale()))
                .unwrap();

            let result = renderer
                .render_winit(|frame| {
                    state.main_state.render(frame, (output_geometry, output_scale))?;

                    let (x, y) = state.main_state.pointer_location().into();
                    // draw the dnd icon if any
                    {
                        let guard = state.main_state.dnd_icon.lock().unwrap();
                        if let Some(ref surface) = *guard {
                            if surface.as_ref().is_alive() {
                                draw_dnd_icon(
                                    frame,
                                    surface,
                                    (x as i32, y as i32).into(),
                                    output_scale,
                                    &log,
                                )?;
                            }
                        }
                    }

                    // draw the cursor as relevant
                    {
                        let mut guard = state.main_state.cursor_status.lock().unwrap();
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
                            cursor_visible = false;
                            draw_cursor(frame, surface, (x as i32, y as i32).into(), output_scale, &log)?;
                        } else {
                            cursor_visible = true;
                        }
                    }

                    #[cfg(feature = "debug")]
                    {
                        let fps = state.backend_data.fps.avg().round() as u32;
                        draw_fps(frame, &state.backend_data.fps_texture, output_scale as f64, fps)?;
                    }

                    Ok(())
                })
                .map_err(Into::<SwapBuffersError>::into)
                .and_then(|x| x);

            renderer.window().set_cursor_visible(cursor_visible);

            if let Err(SwapBuffersError::ContextLost(err)) = result {
                error!(log, "Critical Rendering Error: {}", err);
                state.main_state.running.store(false, Ordering::SeqCst);
            }
        }

        // Send frame events so that client start drawing their next frame
        let time = start_time.elapsed().as_millis() as u32;
        state.main_state.send_frames(time);
        display.borrow_mut().flush_clients(&mut state);

        state.update(event_loop);

        #[cfg(feature = "debug")]
        state.backend_data.fps.tick();
    }

    Ok(())
}
