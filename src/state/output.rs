use smithay::utils::Rectangle;
use smithay::wayland::output::Output;
use smithay_egui::{EguiFrame, EguiMode, EguiState};
use std::cell::RefCell;
use std::time::Instant;

pub struct OutputState {
    fps: fps_ticker::Fps,
    egui_state: RefCell<EguiState>,
}

impl Default for OutputState {
    fn default() -> Self {
        Self {
            fps: Default::default(),
            egui_state: RefCell::new(EguiState::new(EguiMode::Reactive)),
        }
    }
}

impl OutputState {
    pub fn from_output(seat: &Output) -> &Self {
        seat.user_data().insert_if_missing(Self::default);
        seat.user_data().get::<Self>().unwrap()
    }

    pub fn fps_tick(&self) {
        self.fps.tick();
    }

    pub fn egui_frame(&self, output: &Output, start_time: &Instant) -> EguiFrame {
        let size = output.current_mode().unwrap().size;
        let scale = output.current_scale();

        self.egui_state.borrow_mut().run(
            |ctx| {
                egui::Area::new("main")
                    .anchor(egui::Align2::LEFT_TOP, (10.0, 10.0))
                    .show(ctx, |_ui| {});
            },
            Rectangle::from_loc_and_size((0, 0), size.to_logical(scale)),
            scale as f64,
            1.0,
            start_time,
            Default::default(),
        )
    }
}
