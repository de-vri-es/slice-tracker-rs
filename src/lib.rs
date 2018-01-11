#![feature(pointer_methods)]
#![feature(universal_impl_trait)]

mod file_tracker;
mod slice;
mod slice_tracker;
mod source_location;

pub use slice_tracker::SliceTracker;
pub use source_location::SourceLocation;
pub use file_tracker::FileSliceTracker;
