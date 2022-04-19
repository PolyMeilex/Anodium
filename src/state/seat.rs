use std::cell::Cell;

use smithay::{
    utils::{Logical, Point},
    wayland::seat::Seat,
};

#[derive(Debug, Default)]
pub struct SeatState {
    pointer_pos: Cell<Point<f64, Logical>>,
}

impl SeatState {
    pub fn from_seat(seat: &Seat) -> &Self {
        seat.user_data().insert_if_missing(Self::default);
        seat.user_data().get::<Self>().unwrap()
    }

    pub fn pointer_pos(&self) -> Point<f64, Logical> {
        self.pointer_pos.get()
    }

    pub fn set_pointer_pos(&self, pointer_pos: Point<f64, Logical>) {
        self.pointer_pos.set(pointer_pos);
    }
}
