use std::{
    cell::RefCell,
    os::unix::io::RawFd,
    rc::Rc,
    sync::{Arc, Mutex},
};

use smithay::{
    backend::{
        egl::{EGLContext, EGLDisplay},
        input::InputEvent,
        renderer::{gles2::Gles2Renderer, Bind, Unbind},
        x11::{WindowBuilder, X11Backend, X11Event, X11Handle, X11Surface},
    },
    output::{Mode, PhysicalProperties},
    reexports::{
        calloop::{ping, EventLoop},
        gbm,
        wayland_server::DisplayHandle,
    },
};

use super::BackendHandler;
use crate::{NewOutputDescriptor, OutputId};

pub const OUTPUT_NAME: &str = "x11";

struct OutputSurface {
    surface: X11Surface,
    window: smithay::backend::x11::Window,

    output_id: OutputId,
    mode: Mode,

    rerender: bool,
}

struct OutputSurfaceBuilder {
    surface: X11Surface,
    window: smithay::backend::x11::Window,
    id: u64,
}

impl OutputSurfaceBuilder {
    fn new(
        handle: &X11Handle,
        device: Arc<Mutex<gbm::Device<RawFd>>>,
        context: &EGLContext,
        id: u64,
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

        Self {
            surface,
            window,
            id,
        }
    }

    fn build(self) -> (OutputSurface, NewOutputDescriptor) {
        let size = {
            let s = self.window.size();
            (s.w as i32, s.h as i32).into()
        };

        let mode = Mode {
            size,
            refresh: 60_000,
        };

        let physical_properties = PhysicalProperties {
            size: (0, 0).into(),
            subpixel: smithay::output::Subpixel::Unknown,
            make: "Smithay".into(),
            model: "Winit".into(),
        };

        let output_id = crate::OutputId { id: self.id };
        let output = NewOutputDescriptor {
            id: output_id,
            name: "X11".to_string(),
            physical_properties,
            prefered_mode: mode,
            possible_modes: vec![mode],
            transform: smithay::utils::Transform::Normal,
        };

        (
            OutputSurface {
                surface: self.surface,
                window: self.window,
                mode,

                output_id,

                rerender: true,
            },
            output,
        )
    }
}

pub fn run_x11<D>(
    event_loop: &mut EventLoop<'static, D>,
    display: &DisplayHandle,
    handler: &mut D,
) -> Result<(), ()>
where
    D: BackendHandler + 'static,
{
    let backend = X11Backend::new(None).expect("Failed to initilize X11 backend");
    let handle = backend.handle();

    // Obtain the DRM node the X server uses for direct rendering.
    let (_, fd) = handle
        .drm_node()
        .expect("Could not get DRM node used by X server");

    // Create the gbm device for buffer allocation.
    let device = gbm::Device::new(fd).expect("Failed to create gbm device");
    // Initialize EGL using the GBM device.
    let egl = unsafe { EGLDisplay::new(&device, None).expect("Failed to create EGLDisplay") };
    // Create the OpenGL context
    let context = EGLContext::new(&egl, None).expect("Failed to create EGLContext");

    let device = Arc::new(Mutex::new(device));

    let x11_outputs = vec![
        OutputSurfaceBuilder::new(&handle, device.clone(), &context, 0),
        OutputSurfaceBuilder::new(&handle, device, &context, 1),
    ];

    let renderer =
        unsafe { Gles2Renderer::new(context, None) }.expect("Failed to initialize renderer");
    let renderer = Rc::new(RefCell::new(renderer));

    new_x11_window(display, event_loop, handler, backend, renderer, x11_outputs)
}

fn new_x11_window<D>(
    _display: &DisplayHandle,

    event_loop: &mut EventLoop<'static, D>,
    handler: &mut D,

    backend: X11Backend,
    renderer: Rc<RefCell<Gles2Renderer>>,
    x11_outputs: Vec<OutputSurfaceBuilder>,
) -> Result<(), ()>
where
    D: BackendHandler + 'static,
{
    let surface_datas: Vec<_> = x11_outputs
        .into_iter()
        .map(|o| o.build())
        .map(|(o, new)| {
            handler.output_created(new);
            o
        })
        .collect();

    handler.start_compositor();

    let surface_datas = Rc::new(RefCell::new(surface_datas));

    info!("Initialization completed, starting the main loop.");

    let (render, source) = ping::make_ping().unwrap();

    event_loop
        .handle()
        .insert_source(source, {
            let surface_datas = surface_datas.clone();
            move |_: (), _timer_handle, handler| {
                let mut renderer = renderer.borrow_mut();
                let surface_datas = &mut *surface_datas.borrow_mut();

                for surface_data in surface_datas.iter_mut() {
                    if surface_data.rerender {
                        surface_data.rerender = false;
                    } else {
                        continue;
                    }

                    let (buffer, age) = surface_data
                        .surface
                        .buffer()
                        .expect("gbm device was destroyed");
                    if let Err(err) = renderer.bind(buffer) {
                        error!("Error while binding buffer: {}", err);
                    }

                    let res: Result<(), ()> = {
                        handler
                            .output_render(
                                &mut renderer,
                                &surface_data.output_id,
                                age as usize,
                                None,
                            )
                            .ok();
                        Ok(())
                    };

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
                        Err(_) => {
                            todo!();
                            // if let SwapBuffersError::ContextLost(err) = err.into() {
                            //     error!("Critical Rendering Error: {}", err);
                            //     cb(BackendEvent::CloseCompositor {}, ddata.reborrow());
                            // }
                        }
                    }

                    handler.send_frames(&surface_data.output_id);
                }
            }
        })
        .unwrap();

    render.ping();

    event_loop
        .handle()
        .insert_source(backend, move |event, _, handler| {
            let mut surface_datas = surface_datas.borrow_mut();

            match event {
                X11Event::CloseRequested { .. } => {
                    handler.close_compositor();
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

                    let mode = Mode {
                        size,
                        refresh: 60_000,
                    };

                    surface_data.mode = mode;

                    handler.output_mode_updated(&surface_data.output_id, mode);

                    surface_data.rerender = true;
                    render.ping();
                }

                X11Event::PresentCompleted { window_id, .. }
                | X11Event::Refresh { window_id, .. } => {
                    let surface_data = surface_datas
                        .iter_mut()
                        .find(|sd| sd.window.id() == window_id)
                        .unwrap();

                    surface_data.rerender = true;
                    render.ping();
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

                        handler.process_input_event(event, Some(&surface_data.output_id));
                    }
                }
            }
        })
        .expect("Failed to insert X11 Backend into event loop");

    Ok(())
}
