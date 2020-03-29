// Copyright (c) 2018, Maarten de Vries <maarten@de-vri.es>
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
// * Redistributions of source code must retain the above copyright notice, this
//   list of conditions and the following disclaimer.
//
// * Redistributions in binary form must reproduce the above copyright notice,
//   this list of conditions and the following disclaimer in the documentation
//   and/or other materials provided with the distribution.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
// AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
// IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
// DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE
// FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL
// DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
// SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER
// CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY,
// OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

/// Generalization of slices and str.
pub trait Slice {
	type Element;

	fn start_ptr(&self) -> *const Self::Element;
	fn len(&self) -> usize;

	fn is_empty(&self) -> bool {
		self.len() == 0
	}

	fn end_ptr(&self) -> *const Self::Element {
		unsafe { self.start_ptr().add(self.len()) }
	}
}

impl<T> Slice for [T] {
	type Element = T;

	fn len(&self) -> usize {
		self.len()
	}

	fn start_ptr(&self) -> *const Self::Element {
		self.as_ptr()
	}
}

impl Slice for str {
	type Element = u8;

	fn len(&self) -> usize {
		self.len()
	}

	fn start_ptr(&self) -> *const Self::Element {
		self.as_ptr()
	}
}

/// Trait for things that can be borrowed as a slice, including slices themselves.
pub trait BorrowSlice {
	type Slice: Slice + ?Sized;

	fn borrow_slice(&self) -> &Self::Slice;

	fn len(&self) -> usize {
		self.borrow_slice().len()
	}

	fn is_empty(&self) -> bool {
		self.borrow_slice().is_empty()
	}

	fn start_ptr(&self) -> *const <Self::Slice as Slice>::Element {
		self.borrow_slice().start_ptr()
	}

	fn end_ptr(&self) -> *const <Self::Slice as Slice>::Element {
		self.borrow_slice().end_ptr()
	}
}

impl<'a, T: 'a> BorrowSlice for &'a [T] {
	type Slice = [T];

	fn borrow_slice(&self) -> &[T] {
		self
	}
}

impl<'a> BorrowSlice for &'a str {
	type Slice = str;

	fn borrow_slice(&self) -> &str {
		self
	}
}

impl<T> BorrowSlice for Vec<T> {
	type Slice = [T];

	fn borrow_slice(&self) -> &[T] {
		self
	}
}

impl BorrowSlice for String {
	type Slice = str;

	fn borrow_slice(&self) -> &str {
		self
	}
}

impl<T> BorrowSlice for Box<[T]> {
	type Slice = [T];

	fn borrow_slice(&self) -> &[T] {
		self
	}
}

impl<T> BorrowSlice for std::rc::Rc<[T]> {
	type Slice = [T];

	fn borrow_slice(&self) -> &[T] {
		self
	}
}

impl<T> BorrowSlice for std::sync::Arc<[T]> {
	type Slice = [T];

	fn borrow_slice(&self) -> &[T] {
		self
	}
}
