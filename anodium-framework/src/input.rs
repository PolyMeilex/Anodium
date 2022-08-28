use smithay::{
    backend::input::{self, Axis, AxisSource, Event, InputBackend, PointerAxisEvent},
    input::pointer::AxisFrame,
};

pub fn basic_axis_frame<I: InputBackend>(event: &I::PointerAxisEvent) -> AxisFrame {
    let mut frame = AxisFrame::new(event.time()).source(event.source());

    handle_axis::<I>(&mut frame, input::Axis::Horizontal, event);
    handle_axis::<I>(&mut frame, input::Axis::Vertical, event);

    frame
}

fn handle_axis<I: InputBackend>(frame: &mut AxisFrame, axis: Axis, event: &I::PointerAxisEvent) {
    let mut vertical_amount = event
        .amount(axis)
        .unwrap_or_else(|| event.amount_discrete(axis).unwrap_or(0.0) * 3.0);

    // For touchpad let's reverse
    if event.source() == AxisSource::Finger {
        vertical_amount *= -1.0;
    }

    if vertical_amount != 0.0 {
        *frame = frame.value(axis, vertical_amount);

        if let Some(discrete) = event.amount_discrete(axis) {
            *frame = frame.discrete(axis, discrete as i32);
        }
    } else if event.source() == AxisSource::Finger {
        *frame = frame.stop(axis);
    }
}
