use std::sync::PoisonError;

pub trait IgnorePoisoned<T> {
    fn ignore_poisoned(self) -> T;
}

impl<T> IgnorePoisoned<T> for Result<T, PoisonError<T>> {
    fn ignore_poisoned(self) -> T {
        self.expect("object should not be poisoned")
    }
}
