use std::{
    cell::RefCell,
    rc::Rc,
    sync::{Arc, Mutex},
    time::Duration,
};

use smithay::{
    backend::{
        drm::DrmNode,
        egl::{EGLContext, EGLDisplay},
        renderer::{gles2::Gles2Renderer, Bind, ImportEgl, Renderer, Transform, Unbind},
        x11::{WindowBuilder, X11Backend, X11Event, X11Handle, X11Input, X11Surface},
        SwapBuffersError,
    },
    reexports::{
        calloop::{channel, timer::Timer, EventLoop},
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

use crate::{output_map::Output, render::renderer::RenderFrame};

use super::{BackendEvent, BackendRequest};

pub const OUTPUT_NAME: &str = "x11";

struct OutputSurface {
    surface: X11Surface,
    window: smithay::backend::x11::Window,

    fps: fps_ticker::Fps,

    _output_name: String,
    output: Output,
    mode: Mode,

    rerender: bool,
}

struct OutputSurfaceBuilder {
    surface: X11Surface,
    window: smithay::backend::x11::Window,
}

impl OutputSurfaceBuilder {
    fn new(
        handle: &X11Handle,
        device: Arc<Mutex<gbm::Device<DrmNode>>>,
        context: &EGLContext,
    ) -> Self {
        let window = WindowBuilder::new()
            .title("Anodium")
            .build(handle)
            .expect("Failed to create first window");

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

        Self { surface, window }
    }

    fn build(self, display: &mut Display, renderer: &mut Gles2Renderer) -> OutputSurface {
        let size = {
            let s = self.window.size();
            (s.w as i32, s.h as i32).into()
        };

        let mode = Mode {
            size,
            refresh: 60_000,
        };

        let mut imgui = imgui::Context::create();
        {
            imgui.set_ini_filename(None);
            let io = imgui.io_mut();
            io.display_framebuffer_scale = [1.0f32, 1.0f32];
            io.display_size = [size.w as f32, size.h as f32];
        }

        let imgui_pipeline = renderer
            .with_context(|_, gles| imgui_smithay_renderer::Renderer::new(gles, &mut imgui))
            .unwrap();

        let output = Output::new(
            OUTPUT_NAME,
            Default::default(),
            display,
            PhysicalProperties {
                size: (0, 0).into(),
                subpixel: wl_output::Subpixel::Unknown,
                make: "Smithay".into(),
                model: "Winit".into(),
            },
            mode,
            vec![mode],
            imgui,
            imgui_pipeline,
            // TODO: output should always have a workspace
            "Unknown".into(),
            slog_scope::logger(),
        );

        OutputSurface {
            surface: self.surface,
            window: self.window,

            fps: fps_ticker::Fps::default(),
            mode,
            _output_name: "X11(1)".into(),
            output,

            rerender: true,
        }
    }
}

pub fn run_x11<F, IF, D>(
    display: Rc<RefCell<Display>>,

    event_loop: &mut EventLoop<'static, D>,
    state: &mut D,

    rx: channel::Channel<BackendRequest>,

    cb: F,
    input_cb: IF,
) -> Result<(), ()>
where
    F: FnMut(BackendEvent, DispatchData) + 'static,
    IF: FnMut(InputEvent<X11Input>, &Output, DispatchData) + 'static,
    D: 'static,
{
    event_loop
        .handle()
        .insert_source(rx, move |event, _, _| match event {
            channel::Event::Msg(event) => match event {
                BackendRequest::ChangeVT(_) => {}
            },
            channel::Event::Closed => {}
        })
        .unwrap();

    let cb = Rc::new(RefCell::new(cb));
    let input_cb = Rc::new(RefCell::new(input_cb));

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

    let device = Arc::new(Mutex::new(device));

    let x11_outputs = vec![
        OutputSurfaceBuilder::new(&handle, device.clone(), &context),
        OutputSurfaceBuilder::new(&handle, device, &context),
    ];

    let renderer = unsafe { Gles2Renderer::new(context, slog_scope::logger()) }
        .expect("Failed to initialize renderer");
    let renderer = Rc::new(RefCell::new(renderer));

    new_x11_window(
        display,
        event_loop,
        state,
        backend,
        renderer,
        x11_outputs,
        cb,
        input_cb,
    )
}

fn new_x11_window<F, IF, D>(
    display: Rc<RefCell<Display>>,

    event_loop: &mut EventLoop<'static, D>,
    state: &mut D,

    backend: X11Backend,
    renderer: Rc<RefCell<Gles2Renderer>>,
    x11_outputs: Vec<OutputSurfaceBuilder>,

    cb: Rc<RefCell<F>>,
    input_cb: Rc<RefCell<IF>>,
) -> Result<(), ()>
where
    F: FnMut(BackendEvent, DispatchData) + 'static,
    IF: FnMut(InputEvent<X11Input>, &Output, DispatchData) + 'static,
    D: 'static,
{
    let mut ddata = DispatchData::wrap(state);

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

    let surface_datas: Vec<_> = x11_outputs
        .into_iter()
        .map(|o| o.build(&mut display.borrow_mut(), &mut renderer.borrow_mut()))
        .collect();

    for surface_data in surface_datas.iter() {
        (cb.borrow_mut())(
            BackendEvent::RequestOutputConfigure {
                output: surface_data.output.clone(),
            },
            ddata.reborrow(),
        );
        (cb.borrow_mut())(
            BackendEvent::OutputCreated {
                output: surface_data.output.clone(),
            },
            ddata.reborrow(),
        );
    }

    (cb.borrow_mut())(BackendEvent::StartCompositor, ddata.reborrow());

    let surface_datas = Rc::new(RefCell::new(surface_datas));

    info!("Initialization completed, starting the main loop.");

    let timer = Timer::new().unwrap();
    let timer_handle = timer.handle();

    event_loop
        .handle()
        .insert_source(timer, {
            let surface_datas = surface_datas.clone();
            let cb = cb.clone();
            move |_: (), _timer_handle, state| {
                let mut ddata = DispatchData::wrap(state);

                let mut renderer = renderer.borrow_mut();
                let mut cb = cb.borrow_mut();
                let surface_datas = &mut *surface_datas.borrow_mut();

                for surface_data in surface_datas.iter_mut() {
                    if surface_data.rerender {
                        surface_data.rerender = false;
                    } else {
                        continue;
                    }

                    let (buffer, _age) = surface_data
                        .surface
                        .buffer()
                        .expect("gbm device was destroyed");
                    if let Err(err) = renderer.bind(buffer) {
                        error!("Error while binding buffer: {}", err);
                    }

                    let res = renderer.render(
                        surface_data.mode.size,
                        Transform::Normal,
                        |renderer, frame| {
                            let mut frame = RenderFrame {
                                transform: Transform::Normal,
                                renderer,
                                frame,
                            };

                            surface_data.output.update_fps(surface_data.fps.avg());

                            cb(
                                BackendEvent::OutputRender {
                                    frame: &mut frame,
                                    output: &surface_data.output,
                                    pointer_image: None,
                                },
                                ddata.reborrow(),
                            );
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

                    surface_data.fps.tick();
                }

                cb(BackendEvent::SendFrames, ddata);
            }
        })
        .unwrap();
    timer_handle.add_timeout(Duration::ZERO, ());

    event_loop
        .handle()
        .insert_source(backend, move |event, _, state| {
            let mut ddata = DispatchData::wrap(state);

            let mut surface_datas = surface_datas.borrow_mut();

            match event {
                X11Event::CloseRequested { .. } => {
                    let mut cb = cb.borrow_mut();
                    cb(BackendEvent::CloseCompositor {}, ddata.reborrow());
                }

                X11Event::Resized {
                    new_size,
                    window_id,
                } => {
                    let surface_data = surface_datas
                        .iter_mut()
                        .find(|sd| sd.window.id() == window_id)
                        .unwrap();

                    let size = (new_size.w as i32, new_size.h as i32).into();
                    let scale_factor = 1.0;

                    let mode = Mode {
                        size,
                        refresh: 60_000,
                    };

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

                    surface_data.rerender = true;
                    timer_handle.add_timeout(Duration::ZERO, ());
                }

                X11Event::PresentCompleted { window_id, .. }
                | X11Event::Refresh { window_id, .. } => {
                    let surface_data = surface_datas
                        .iter_mut()
                        .find(|sd| sd.window.id() == window_id)
                        .unwrap();

                    surface_data.rerender = true;
                    timer_handle.add_timeout(Duration::ZERO, ());
                }

                X11Event::Input(event) => {
                    let id: Option<u32> = match &event {
                        InputEvent::Keyboard { event } => event.window().map(|w| w.as_ref().id()),
                        InputEvent::PointerMotionAbsolute { event } => {
                            event.window().map(|w| w.as_ref().id())
                        }
                        InputEvent::PointerAxis { event } => {
                            event.window().map(|w| w.as_ref().id())
                        }
                        InputEvent::PointerButton { event } => {
                            event.window().map(|w| w.as_ref().id())
                        }
                        _ => None,
                    };

                    if let Some(window_id) = id {
                        let surface_data = surface_datas
                            .iter_mut()
                            .find(|sd| sd.window.id() == window_id)
                            .unwrap();
                        (input_cb.borrow_mut())(event, &surface_data.output, ddata.reborrow());
                    }
                }
            }
        })
        .expect("Failed to insert X11 Backend into event loop");

    Ok(())
}
