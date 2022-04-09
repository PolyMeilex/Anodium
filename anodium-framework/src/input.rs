use smithay::{
    backend::input::{self, Event, InputBackend, PointerAxisEvent},
    reexports::wayland_server::protocol::wl_pointer,
    wayland::seat::AxisFrame,
};

pub fn basic_axis_frame<I: InputBackend>(evt: &I::PointerAxisEvent) -> AxisFrame {
    let source = match evt.source() {
        input::AxisSource::Continuous => wl_pointer::AxisSource::Continuous,
        input::AxisSource::Finger => wl_pointer::AxisSource::Finger,
        input::AxisSource::Wheel | input::AxisSource::WheelTilt => wl_pointer::AxisSource::Wheel,
    };
    let horizontal_amount = evt
        .amount(input::Axis::Horizontal)
        .unwrap_or_else(|| evt.amount_discrete(input::Axis::Horizontal).unwrap() * 3.0);
    let vertical_amount = evt
        .amount(input::Axis::Vertical)
        .unwrap_or_else(|| evt.amount_discrete(input::Axis::Vertical).unwrap() * 3.0);
    let horizontal_amount_discrete = evt.amount_discrete(input::Axis::Horizontal);
    let vertical_amount_discrete = evt.amount_discrete(input::Axis::Vertical);

    let mut frame = AxisFrame::new(evt.time()).source(source);
    if horizontal_amount != 0.0 {
        frame = frame.value(wl_pointer::Axis::HorizontalScroll, horizontal_amount);
        if let Some(discrete) = horizontal_amount_discrete {
            frame = frame.discrete(wl_pointer::Axis::HorizontalScroll, discrete as i32);
        }
    } else if source == wl_pointer::AxisSource::Finger {
        frame = frame.stop(wl_pointer::Axis::HorizontalScroll);
    }

    if vertical_amount != 0.0 {
        frame = frame.value(wl_pointer::Axis::VerticalScroll, vertical_amount);
        if let Some(discrete) = vertical_amount_discrete {
            frame = frame.discrete(wl_pointer::Axis::VerticalScroll, discrete as i32);
        }
    } else if source == wl_pointer::AxisSource::Finger {
        frame = frame.stop(wl_pointer::Axis::VerticalScroll);
    }

    frame
}
