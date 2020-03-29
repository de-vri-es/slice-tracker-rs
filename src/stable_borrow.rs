/// Marker trait to indicate that borrowed references are stable,
/// even when the owning object is moved.
pub unsafe trait StableBorrow {}

unsafe impl<'a> StableBorrow for &'a str {}
unsafe impl<'a, T> StableBorrow for &'a [T] {}
unsafe impl StableBorrow for String {}
unsafe impl StableBorrow for std::path::PathBuf {}
unsafe impl<T> StableBorrow for Vec<T> {}
unsafe impl<T> StableBorrow for Box<T> {}
