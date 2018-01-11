pub trait PointerRange {
	type Type;

	fn start_ptr(&self) -> *const Self::Type;
	fn end_ptr(&self)   -> *const Self::Type;
}

impl<T: AsRef<[u8]>> PointerRange for T {
	type Type = u8;
	fn start_ptr(&self) -> *const u8 { self.as_ref().as_ptr() }
	fn end_ptr(&self)   -> *const u8 { unsafe { self.as_ref().as_ptr().add(self.as_ref().len()) } }
}
