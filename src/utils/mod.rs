use std::fmt;

pub mod glow;
pub mod iterators;

pub trait LogResult {
    /// Log if error,
    /// do nothing otherwhise
    fn log_err(self, label: &str) -> Self;
}

impl<D, E: fmt::Debug> LogResult for Result<D, E> {
    fn log_err(self, label: &str) -> Self {
        if let Err(ref err) = self {
            error!("{} {:?}", label, err);
        }

        self
    }
}
