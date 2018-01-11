pub trait Slice {
	type PtrType;

	fn len(&self)       -> usize;
	fn is_empty(&self)  -> bool;
	fn start_ptr(&self) -> *const Self::PtrType;

	fn end_ptr(&self)   -> *const Self::PtrType {
		unsafe { self.start_ptr().add(self.len()) }
	}
}

impl<T> Slice for [T] {
	type PtrType = T;

	fn len(&self) -> usize {
		self.len()
	}

	fn is_empty(&self) -> bool {
		self.is_empty()
	}

	fn start_ptr(&self) -> *const Self::PtrType {
		self.as_ptr()
	}
}

impl Slice for str {
	type PtrType = u8;

	fn len(&self) -> usize {
		self.len()
	}

	fn is_empty(&self) -> bool {
		self.is_empty()
	}

	fn start_ptr(&self) -> *const Self::PtrType {
		self.as_ptr()
	}
}
