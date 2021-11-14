#[derive(Clone, Copy, Debug)]
pub enum EnterExitAnimation {
    Enter(f64),
    Exit(f64),
    EnterDone,
    ExitDone,
    None,
}

impl Default for EnterExitAnimation {
    fn default() -> Self {
        Self::None
    }
}

impl EnterExitAnimation {
    pub fn value(&self) -> f64 {
        let v = match *self {
            Self::Enter(v) => v,
            Self::Exit(v) => v,
            Self::EnterDone => 1.0,
            Self::ExitDone => 0.0,
            Self::None => 0.0,
        };

        fn curve(v: f64) -> f64 {
            if v < 0.0 {
                0.0
            } else if (0.0..1.0).contains(&v) {
                -(v * v) + 2.0 * v
            } else {
                1.0
            }
        }

        curve(v)
    }

    pub fn is_exiting(&self) -> bool {
        matches!(self, Self::Exit(_))
    }

    pub fn exited(&self) -> bool {
        matches!(self, Self::ExitDone)
    }

    pub fn update(&mut self, delta: f64, toplevel_alive: bool) {
        let k = 4.0 * delta;

        *self = match *self {
            Self::Enter(n) => {
                if toplevel_alive {
                    if n < 1.0 {
                        Self::Enter(n + k)
                    } else {
                        Self::EnterDone
                    }
                } else {
                    Self::Exit(n)
                }
            }
            Self::Exit(n) => {
                if n > 0.0 {
                    Self::Exit(n - k)
                } else {
                    Self::ExitDone
                }
            }
            Self::EnterDone => {
                if toplevel_alive {
                    Self::EnterDone
                } else {
                    Self::Exit(1.0)
                }
            }
            a => a,
        }
    }
}
