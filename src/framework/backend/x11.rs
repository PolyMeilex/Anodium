use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use smithay::{
    backend::{
        egl::{EGLContext, EGLDisplay},
        renderer::{gles2::Gles2Renderer, Bind, ImportEgl, Renderer, Transform, Unbind},
        x11::{WindowBuilder, X11Backend, X11Event, X11Input, X11Surface},
        SwapBuffersError,
    },
    reexports::{
        calloop::{timer::Timer, EventLoop},
        gbm,
        wayland_server::{protocol::wl_output, Display},
    },
    wayland::output::{Mode, PhysicalProperties},
};
use smithay::{
    backend::{input::InputEvent, renderer::ImportDma},
    reexports::wayland_server::DispatchData,
    wayland::dmabuf::init_dmabuf_global,
};

use crate::{
    output_map::Output,
    render::{draw_fps, renderer::RenderFrame},
};

use super::BackendEvent;

pub const OUTPUT_NAME: &str = "x11";

struct OutputSurfaceData {
    surface: X11Surface,
    fps: fps_ticker::Fps,
    imgui: Option<imgui::SuspendedContext>,
    imgui_pipeline: imgui_smithay_renderer::Renderer,

    output_name: String,
    output: Output,
    mode: Mode,
}

pub fn run_x11<F, IF, D>(
    display: Rc<RefCell<Display>>,

    event_loop: &mut EventLoop<'static, D>,
    state: &mut D,

    mut cb: F,
    mut input_cb: IF,
) -> Result<(), ()>
where
    F: FnMut(BackendEvent, DispatchData) + 'static,
    IF: FnMut(InputEvent<X11Input>, DispatchData) + 'static,
    D: 'static,
{
    let mut ddata = DispatchData::wrap(state);

    let backend = X11Backend::new(slog_scope::logger()).expect("Failed to initilize X11 backend");
    let handle = backend.handle();

    // Obtain the DRM node the X server uses for direct rendering.
    let drm_node = handle
        .drm_node()
        .expect("Could not get DRM node used by X server");

    // Create the gbm device for buffer allocation.
    let device = gbm::Device::new(drm_node).expect("Failed to create gbm device");
    // Initialize EGL using the GBM device.
    let egl = EGLDisplay::new(&device, slog_scope::logger()).expect("Failed to create EGLDisplay");
    // Create the OpenGL context
    let context = EGLContext::new(&egl, slog_scope::logger()).expect("Failed to create EGLContext");

    let window = WindowBuilder::new()
        .title("Anvil")
        .build(&handle)
        .expect("Failed to create first window");

    let device = Arc::new(Mutex::new(device));

    // Create the surface for the window.
    let surface = handle
        .create_surface(
            &window,
            device,
            context
                .dmabuf_render_formats()
                .iter()
                .map(|format| format.modifier),
        )
        .expect("Failed to create X11 surface");

    let renderer = unsafe { Gles2Renderer::new(context, slog_scope::logger()) }
        .expect("Failed to initialize renderer");
    let renderer = Rc::new(RefCell::new(renderer));

    if renderer
        .borrow_mut()
        .bind_wl_display(&*display.borrow())
        .is_ok()
    {
        info!("EGL hardware-acceleration enabled");
        let dmabuf_formats = renderer
            .borrow_mut()
            .dmabuf_formats()
            .cloned()
            .collect::<Vec<_>>();
        let renderer = renderer.clone();
        init_dmabuf_global(
            &mut *display.borrow_mut(),
            dmabuf_formats,
            move |buffer, _| renderer.borrow_mut().import_dmabuf(buffer).is_ok(),
            slog_scope::logger(),
        );
    }

    let size = {
        let s = window.size();

        (s.w as i32, s.h as i32).into()
    };

    let mode = Mode {
        size,
        refresh: 60_000,
    };

    let output = Output::new(
        OUTPUT_NAME,
        Default::default(),
        &mut *display.borrow_mut(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: wl_output::Subpixel::Unknown,
            make: "Smithay".into(),
            model: "Winit".into(),
        },
        mode,
        // TODO: output should always have a workspace
        "Unknown".into(),
        slog_scope::logger(),
    );

    cb(
        BackendEvent::OutputCreated {
            output: output.clone(),
        },
        ddata.reborrow(),
    );

    cb(BackendEvent::StartCompositor, ddata.reborrow());

    let mut imgui = imgui::Context::create();
    {
        imgui.set_ini_filename(None);
        let io = imgui.io_mut();
        io.display_framebuffer_scale = [1.0f32, 1.0f32];
        io.display_size = [size.w as f32, size.h as f32];
    }

    let imgui_pipeline = renderer
        .borrow_mut()
        .with_context(|_, gles| imgui_smithay_renderer::Renderer::new(gles, &mut imgui))
        .unwrap();

    let surface_data = OutputSurfaceData {
        surface,
        fps: fps_ticker::Fps::default(),
        imgui: Some(imgui.suspend()),
        imgui_pipeline,
        mode,
        output_name: "X11(1)".into(),
        output,
    };

    let surface_data = Rc::new(RefCell::new(surface_data));

    info!("Initialization completed, starting the main loop.");

    let timer = Timer::new().unwrap();
    let timer_handle = timer.handle();

    let cb = Rc::new(RefCell::new(cb));

    event_loop
        .handle()
        .insert_source(timer, {
            let surface_data = surface_data.clone();
            let cb = cb.clone();
            move |_: (), _, state| {
                let mut ddata = DispatchData::wrap(state);

                let mut renderer = renderer.borrow_mut();
                let mut cb = cb.borrow_mut();
                let surface_data = &mut *surface_data.borrow_mut();

                let (buffer, _age) = surface_data
                    .surface
                    .buffer()
                    .expect("gbm device was destroyed");
                if let Err(err) = renderer.bind(buffer) {
                    error!("Error while binding buffer: {}", err);
                }

                let mut imgui = surface_data.imgui.take().unwrap().activate().unwrap();

                let res = renderer.render(
                    surface_data.mode.size,
                    Transform::Flipped180,
                    |renderer, frame| {
                        let ui = imgui.frame();

                        {
                            let mut frame = RenderFrame {
                                transform: Transform::Flipped180,
                                renderer,
                                frame,
                                imgui: &ui,
                            };

                            cb(
                                BackendEvent::OutputRender {
                                    frame: &mut frame,
                                    output: &surface_data.output,
                                    pointer_image: None,
                                },
                                ddata.reborrow(),
                            );
                        }

                        draw_fps(&ui, 1.0, surface_data.fps.avg());

                        let draw_data = ui.render();

                        renderer
                            .with_context(|_renderer, gles| {
                                surface_data.imgui_pipeline.render(
                                    Transform::Flipped180,
                                    gles,
                                    draw_data,
                                );
                            })
                            .unwrap();
                    },
                );

                match res {
                    Ok(_) => {
                        // Unbind the buffer
                        if let Err(err) = renderer.unbind() {
                            error!("Error while unbinding buffer: {}", err);
                        }

                        // Submit the buffer
                        if let Err(err) = surface_data.surface.submit() {
                            error!("Error submitting buffer for display: {}", err);
                        }
                    }
                    Err(err) => {
                        if let SwapBuffersError::ContextLost(err) = err.into() {
                            error!("Critical Rendering Error: {}", err);
                            cb(BackendEvent::CloseCompositor {}, ddata.reborrow());
                        }
                    }
                }

                cb(BackendEvent::SendFrames, ddata);

                surface_data.fps.tick();
                surface_data.imgui = Some(imgui.suspend());
            }
        })
        .unwrap();
    timer_handle.add_timeout(Duration::ZERO, ());

    event_loop
        .handle()
        .insert_source(backend, move |event, _window, state| {
            let mut ddata = DispatchData::wrap(state);

            match event {
                X11Event::CloseRequested => {
                    let mut cb = cb.borrow_mut();
                    cb(BackendEvent::CloseCompositor {}, ddata.reborrow());
                }

                X11Event::Resized(size) => {
                    let mut surface_data = surface_data.borrow_mut();

                    let size = (size.w as i32, size.h as i32).into();
                    let scale_factor = 1.0;

                    let mode = Mode {
                        size,
                        refresh: 60_000,
                    };

                    {
                        let mut imgui = surface_data.imgui.take().unwrap().activate().unwrap();
                        let io = imgui.io_mut();
                        io.display_framebuffer_scale = [scale_factor as f32, scale_factor as f32];
                        io.display_size = [size.w as f32, size.h as f32];
                        surface_data.imgui = Some(imgui.suspend());
                    }

                    surface_data.mode = mode;
                    surface_data.output.update_mode(mode);
                    surface_data.output.update_scale(scale_factor);

                    let mut cb = cb.borrow_mut();
                    cb(
                        BackendEvent::OutputModeUpdate {
                            output: &surface_data.output,
                        },
                        ddata.reborrow(),
                    );

                    timer_handle.add_timeout(Duration::ZERO, ());
                }

                X11Event::PresentCompleted | X11Event::Refresh => {
                    timer_handle.add_timeout(Duration::ZERO, ());
                }

                X11Event::Input(event) => {
                    input_cb(event, ddata.reborrow());
                }
            }
        })
        .expect("Failed to insert X11 Backend into event loop");

    Ok(())
}
