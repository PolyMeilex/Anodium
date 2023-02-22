use std::{cell::RefCell, rc::Rc, time::Duration};

use smithay::{
    backend::winit::{self, WinitEvent},
    output::{Mode, PhysicalProperties},
    reexports::calloop::{
        timer::{TimeoutAction, Timer},
        EventLoop,
    },
};

use super::BackendHandler;
use crate::{NewOutputDescriptor, OutputId};

pub const OUTPUT_NAME: &str = "winit";

pub fn run_winit<D>(event_loop: &mut EventLoop<'static, D>, handler: &mut D) -> Result<(), ()>
where
    D: BackendHandler + 'static,
{
    let (backend, mut input) = winit::init().map_err(|err| {
        error!("Failed to initialize Winit backend: {}", err);
    })?;
    let backend = Rc::new(RefCell::new(backend));

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
        subpixel: smithay::output::Subpixel::Unknown,
        make: "Smithay".into(),
        model: "Winit".into(),
    };

    let output_id = OutputId { id: 1 };

    let output = NewOutputDescriptor {
        id: output_id,
        physical_properties,
        transform: smithay::utils::Transform::Flipped180,
        name: OUTPUT_NAME.to_owned(),
        prefered_mode: mode,
        possible_modes: vec![mode],
    };

    handler.output_created(output);
    handler.start_compositor();

    info!("Initialization completed, starting the main loop.");

    let timer = Timer::immediate();

    event_loop
        .handle()
        .insert_source(timer, move |_, _, handler| {
            let res = input.dispatch_new_events(|event| match event {
                WinitEvent::Resized { size, .. } => {
                    let mode = Mode {
                        size,
                        refresh: 60_000,
                    };

                    handler.output_mode_updated(&output_id, mode);
                }
                WinitEvent::Input(event) => {
                    handler.process_input_event(event, Some(&output_id));
                }
                _ => {}
            });

            match res {
                Ok(()) => {
                    let mut backend = backend.borrow_mut();

                    if backend.bind().is_ok() {
                        let age = backend.buffer_age().unwrap_or(0);
                        let damage = handler
                            .output_render(backend.renderer(), &output_id, age, None)
                            .unwrap();
                        backend.submit(damage.as_deref()).unwrap();
                    }

                    handler.send_frames(&output_id);

                    TimeoutAction::ToDuration(Duration::from_millis(16))
                }
                Err(winit::WinitError::WindowClosed) => {
                    handler.close_compositor();

                    TimeoutAction::Drop
                }
            }
        })
        .unwrap();

    Ok(())
}
