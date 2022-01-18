use std::{cell::RefCell, rc::Rc, time::Duration};

use smithay::{
    backend::{
        renderer::{ImportDma, ImportEgl},
        winit::{self, WinitEvent},
    },
    reexports::{
        calloop::{channel, timer::Timer, EventLoop},
        wayland_server::{protocol::wl_output, Display},
    },
    wayland::{
        dmabuf::init_dmabuf_global,
        output::{Mode, PhysicalProperties},
    },
};

use super::{BackendHandler, BackendRequest};

use crate::output_manager::{Output, OutputDescriptor};

pub const OUTPUT_NAME: &str = "winit";

pub fn run_winit<D>(
    display: Rc<RefCell<Display>>,

    event_loop: &mut EventLoop<'static, D>,
    handler: &mut D,

    rx: channel::Channel<BackendRequest>,
) -> Result<(), ()>
where
    D: BackendHandler + 'static,
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

    let (backend, mut input) = winit::init(slog_scope::logger()).map_err(|err| {
        crit!("Failed to initialize Winit backend: {}", err);
    })?;
    let backend = Rc::new(RefCell::new(backend));

    if backend
        .borrow_mut()
        .renderer()
        .bind_wl_display(&display.borrow())
        .is_ok()
    {
        info!("EGL hardware-acceleration enabled");
        let dmabuf_formats = backend
            .borrow_mut()
            .renderer()
            .dmabuf_formats()
            .cloned()
            .collect::<Vec<_>>();
        let backend = backend.clone();
        init_dmabuf_global(
            &mut *display.borrow_mut(),
            dmabuf_formats,
            move |buffer, _| {
                backend
                    .borrow_mut()
                    .renderer()
                    .import_dmabuf(buffer)
                    .is_ok()
            },
            slog_scope::logger(),
        );
    };

    let size = backend.borrow().window_size().physical_size;

    /*
     * Initialize the globals
     */

    let mode = Mode {
        size,
        refresh: 60_000,
    };

    let descriptor = OutputDescriptor {
        name: OUTPUT_NAME.to_owned(),
        physical_properties: PhysicalProperties {
            size: (0, 0).into(),
            subpixel: wl_output::Subpixel::Unknown,
            make: "Smithay".into(),
            model: "Winit".into(),
        },
    };

    let mode = handler.ask_for_output_mode(&descriptor, &[mode]);

    let output = Output::new(
        &mut *display.borrow_mut(),
        handler.anodium_protocol(),
        descriptor,
        wl_output::Transform::Flipped180,
        mode,
        vec![mode],
    );

    handler.output_created(output.clone());
    handler.start_compositor();

    info!("Initialization completed, starting the main loop.");

    let timer = Timer::new().unwrap();
    let timer_handle = timer.handle();

    event_loop
        .handle()
        .insert_source(timer, move |_: (), timer_handle, handler| {
            let res = input.dispatch_new_events(|event| match event {
                WinitEvent::Resized { size, .. } => {
                    let mode = Mode {
                        size,
                        refresh: 60_000,
                    };

                    output.change_current_state(Some(mode), None, Some(1), None);
                    handler.output_mode_updated(&output, mode);
                }
                WinitEvent::Input(event) => {
                    handler.process_input_event(event, Some(&output));
                }
                _ => {}
            });

            match res {
                Ok(()) => {
                    let mut backend = backend.borrow_mut();

                    if backend.bind().is_ok() {
                        handler.output_render(backend.renderer(), &output, None);
                        backend.submit(None, 1.0).unwrap();
                    }

                    handler.send_frames();

                    timer_handle.add_timeout(Duration::from_millis(16), ());
                }
                Err(winit::WinitError::WindowClosed) => handler.close_compositor(),
            }
        })
        .unwrap();
    timer_handle.add_timeout(Duration::ZERO, ());

    Ok(())
}
