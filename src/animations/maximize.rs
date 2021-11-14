#![allow(dead_code)]

use smithay::utils::{Logical, Physical, Point, Rectangle, Size};

use crate::render::renderer::RenderFrame;

pub enum MaximizeAnimationState {
    Enter,
    Exit,
    None,
}

impl Default for MaximizeAnimationState {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Default)]
pub struct MaximizeAnimation {
    state: MaximizeAnimationState,
    geometry: Rectangle<f64, Logical>,
    target_geometry: Rectangle<f64, Logical>,

    progress: f64,
}

impl MaximizeAnimation {
    pub fn start(
        &mut self,
        _initial_geometry: Rectangle<i32, Logical>,
        target_geometry: Rectangle<i32, Logical>,
    ) {
        match self.state {
            MaximizeAnimationState::None | MaximizeAnimationState::Exit => {
                self.state = MaximizeAnimationState::Enter;
                self.target_geometry = target_geometry.to_f64();
            }
            _ => {}
        };
    }

    pub fn stop(&mut self) {
        if let MaximizeAnimationState::Enter = self.state {
            self.state = MaximizeAnimationState::Exit;
        }
    }

    pub fn update(&mut self, elapsed: f64) {
        let k = elapsed * 3.0;
        match &mut self.state {
            MaximizeAnimationState::Enter => {
                self.progress = (self.progress + k).min(1.0);
                let margin = 20.0;
                self.geometry = Rectangle::from_loc_and_size(
                    Point::from((margin, margin)),
                    Size::from((
                        (self.target_geometry.size.w - margin * 2.0) * self.progress,
                        (self.target_geometry.size.h - margin * 2.0) * self.progress,
                    )),
                );
            }
            MaximizeAnimationState::Exit => {
                self.progress = (self.progress - k * 2.0).max(0.0);
            }
            _ => {}
        }
    }

    pub fn render(
        &mut self,
        _frame: &mut RenderFrame,

        (_output_geometry, _output_scale): (Rectangle<i32, Physical>, f64),
    ) {
        match &self.state {
            MaximizeAnimationState::Enter | MaximizeAnimationState::Exit => {
                // let quad = self.geometry.to_physical(output_scale);
                // let alpha = self.progress;

                // TODO:
                // frame.quad_pipeline.render(
                //     output_geometry.to_f64(),
                //     quad,
                //     frame.transform,
                //     &frame.context,
                //     alpha as f32 / 2.0,
                // );
            }
            _ => {}
        }
    }
}
