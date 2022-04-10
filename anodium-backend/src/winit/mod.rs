use std::{cell::RefCell, rc::Rc, time::Duration};

use smithay::{
    backend::{
        renderer::{ImportDma, ImportEgl},
        winit::{self, WinitEvent},
    },
    reexports::{
        calloop::{timer::Timer, EventLoop},
        wayland_server::{protocol::wl_output, Display},
    },
    wayland::{
        dmabuf::init_dmabuf_global,
        output::{Mode, Output as SmithayOutput, PhysicalProperties},
    },
};

use super::BackendHandler;

pub const OUTPUT_NAME: &str = "winit";

pub fn run_winit<D>(
    event_loop: &mut EventLoop<'static, D>,
    display: Rc<RefCell<Display>>,
    handler: &mut D,
) -> Result<(), ()>
where
    D: BackendHandler + 'static,
{
    let (backend, mut input) = winit::init(None).map_err(|err| {
        error!("Failed to initialize Winit backend: {}", err);
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
                    .import_dmabuf(buffer, None)
                    .is_ok()
            },
            None,
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

    let physical_properties = PhysicalProperties {
        size: (0, 0).into(),
        subpixel: wl_output::Subpixel::Unknown,
        make: "Smithay".into(),
        model: "Winit".into(),
    };

    let (output, _) = SmithayOutput::new(
        &mut *display.borrow_mut(),
        OUTPUT_NAME.to_owned(),
        physical_properties,
        None,
    );

    output.set_preferred(mode);
    output.change_current_state(
        Some(mode),
        Some(wl_output::Transform::Flipped180),
        None,
        None,
    );

    handler.output_created(output.clone(), vec![mode]);
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
                        let age = backend.buffer_age().unwrap_or(0);
                        let damage = handler
                            .output_render(backend.renderer(), &output, age, None)
                            .unwrap();
                        backend.submit(damage.as_deref(), 1.0).unwrap();
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
