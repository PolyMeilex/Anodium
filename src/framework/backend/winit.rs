use std::{cell::RefCell, rc::Rc, time::Duration};

use smithay::{
    backend::{
        input::InputEvent,
        renderer::{ImportDma, ImportEgl, Transform},
        winit::{self, WinitEvent, WinitInput},
    },
    reexports::{
        calloop::{channel, timer::Timer, EventLoop},
        wayland_server::{protocol::wl_output, DispatchData, Display},
    },
    wayland::{
        dmabuf::init_dmabuf_global,
        output::{Mode, PhysicalProperties},
    },
};

use super::{BackendEvent, BackendRequest};

use crate::{output_map::Output, render::renderer::RenderFrame, render::*, state::Anodium};

pub const OUTPUT_NAME: &str = "winit";

pub fn run_winit<F, IF, D>(
    display: Rc<RefCell<Display>>,

    event_loop: &mut EventLoop<'static, D>,
    state: &mut D,

    rx: channel::Channel<BackendRequest>,

    mut cb: F,
    mut input_cb: IF,
) -> Result<(), ()>
where
    F: FnMut(BackendEvent, DispatchData) + 'static,
    IF: FnMut(InputEvent<WinitInput>, &Output, DispatchData) + 'static,
    D: 'static,
{
    let mut ddata = DispatchData::wrap(state);

    event_loop
        .handle()
        .insert_source(rx, move |event, _, _| match event {
            channel::Event::Msg(event) => match event {
                BackendRequest::ChangeVT(_) => {}
            },
            channel::Event::Closed => {}
        })
        .unwrap();

    let (renderer, mut input) = winit::init(slog_scope::logger()).map_err(|err| {
        crit!("Failed to initialize Winit backend: {}", err);
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
            slog_scope::logger(),
        );
    };

    let size = renderer.borrow().window_size().physical_size;

    /*
     * Initialize the globals
     */

    let mode = Mode {
        size,
        refresh: 60_000,
    };

    let mut output = Output::new(
        OUTPUT_NAME,
        Default::default(),
        &mut *display.borrow_mut(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: wl_output::Subpixel::Unknown,
            make: "Smithay".into(),
            model: "Winit".into(),
        },
        mode.clone(),
        vec![mode],
        // TODO: output should always have a workspace
        "Unknown".into(),
        slog_scope::logger(),
    );

    ddata
        .get::<Anodium>()
        .unwrap()
        .config
        .output_new(output.clone());

    cb(
        BackendEvent::OutputCreated {
            output: output.clone(),
        },
        ddata.reborrow(),
    );

    cb(BackendEvent::StartCompositor, ddata.reborrow());

    info!("imgui!");
    let mut imgui = imgui::Context::create();
    {
        imgui.set_ini_filename(None);
        let io = imgui.io_mut();
        io.display_framebuffer_scale = [1.0f32, 1.0f32];
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
            let mut ddata = DispatchData::wrap(state);

            let res = input.dispatch_new_events(|event| match event {
                WinitEvent::Resized { size, scale_factor } => {
                    {
                        let io = imgui.io_mut();
                        io.display_framebuffer_scale = [scale_factor as f32, scale_factor as f32];
                        io.display_size = [size.w as f32, size.h as f32];
                    }

                    let mode = Mode {
                        size,
                        refresh: 60_000,
                    };

                    output.update_mode(mode);
                    output.update_scale(scale_factor);

                    cb(
                        BackendEvent::OutputModeUpdate { output: &output },
                        ddata.reborrow(),
                    );
                }
                WinitEvent::Input(event) => {
                    input_cb(event, &output, ddata.reborrow());
                }
                _ => {}
            });

            match res {
                Ok(()) => {
                    let mut renderer = renderer.borrow_mut();

                    renderer
                        .render(|renderer, frame| {
                            let ui = imgui.frame();

                            {
                                let mut frame = RenderFrame {
                                    transform: Transform::Normal,
                                    renderer,
                                    frame,
                                    imgui: Some((ui, &imgui_pipeline)),
                                };

                                output.update_fps(fps.avg());

                                cb(
                                    BackendEvent::OutputRender {
                                        frame: &mut frame,
                                        output: &output,
                                        pointer_image: None,
                                    },
                                    ddata.reborrow(),
                                );
                            }

                            /*let draw_data = ui.render();

                            renderer
                                .with_context(|_renderer, gles| {
                                    imgui_pipeline.render(Transform::Normal, gles, draw_data);
                                })
                                .unwrap();*/
                        })
                        .unwrap();

                    cb(BackendEvent::SendFrames, ddata);

                    fps.tick();

                    handle.add_timeout(Duration::from_millis(16), ());
                }
                Err(winit::WinitError::WindowClosed) => {
                    cb(BackendEvent::CloseCompositor, ddata);
                }
            }
        })
        .unwrap();
    timer_handle.add_timeout(Duration::ZERO, ());

    Ok(())
}
