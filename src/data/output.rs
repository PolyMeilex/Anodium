use smithay::wayland::output::Output;

#[derive(Default, Debug)]
pub struct OutputState {
    fps: fps_ticker::Fps,
}

impl OutputState {
    pub fn for_output(seat: &Output) -> &Self {
        seat.user_data().insert_if_missing(Self::default);
        seat.user_data().get::<Self>().unwrap()
    }

    pub fn fps_tick(&self) {
        self.fps.tick();
    }
}
