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

use std::borrow::Cow;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use crate::SliceTracker;

pub enum SourceLocation<'a, Data>
where
	Data: 'a + ?Sized,
{
	/// The source of the data is unknown.
	Unknown,

	/// The data was expanded from other data.
	ExpandedFrom(&'a Data),

	/// The data came from a file.
	File(FileLocation<'a>),
}

/// File location indicating the source of a slice of data.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct FileLocation<'a> {
	pub path: &'a Path,
	pub line: usize,
	pub column: usize,
}

/// Source of a slice of data.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Source<'a, Data>
where
	Data: 'a + ?Sized,
{
	/// Unknown source.
	Unknown,

	/// The data was expanded from other data.
	ExpandedFrom(&'a Data),

	/// The data was read from a file.
	File(PathBuf),
}

/// Search for a subslice, and compute the location as (line, colum) in the larger slice.
fn compute_location(subslice: &[u8], data: &[u8]) -> (usize, usize) {
	let offset = subslice.as_ptr() as usize - data.as_ptr() as usize;
	let mut line_breaks = memchr::memrchr_iter(b'\n', &data[..offset]);
	match line_breaks.next() {
		None => (1, offset + 1),
		Some(i) => (line_breaks.count() + 2, offset - i),
	}
}

/// Read a text file (UTF-8) into a string.
fn read_text_file<P: ?Sized + AsRef<Path>>(path: &P) -> std::io::Result<String> {
	let mut file = File::open(path)?;
	let mut data = String::new();
	file.read_to_string(&mut data)?;
	return Ok(data);
}

/// Read a binary file into a Vec<u8>.
fn read_binary_file<P: ?Sized + AsRef<Path>>(path: &P) -> std::io::Result<Vec<u8>> {
	let mut file = File::open(path)?;
	let mut data = Vec::new();
	file.read_to_end(&mut data)?;
	return Ok(data);
}

pub trait FileTracker<Data: ?Sized> {
	/// Read a file and insert it into the tracker.
	///
	/// Fails if reading the file fails, or if the file is empty.
	fn insert_file(&self, path: impl Into<PathBuf>) -> std::io::Result<&Data>;

	/// Get the source location for a slice of data.
	fn get_source_location<'s, 'd>(&'s self, data: &'d Data) -> Option<SourceLocation<'s, Data>>;
}

impl<'a> FileTracker<str> for SliceTracker<'a, str, Source<'a, str>> {
	fn insert_file(&self, path: impl Into<PathBuf>) -> std::io::Result<&str> {
		let path = path.into();
		let data = read_text_file(&path)?;
		if data.is_empty() {
			Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "file is empty"))
		} else {
			// New strings can't be in the tracker yet, so this should be safe.
			Ok(unsafe { self.insert_unsafe(Cow::Owned(data), Source::File(path)) })
		}
	}

	fn get_source_location(&self, data: &str) -> Option<SourceLocation<str>> {
		let (whole_slice, source) = self.get(data)?;
		Some(match source {
			Source::Unknown => SourceLocation::Unknown,
			Source::ExpandedFrom(sources) => SourceLocation::ExpandedFrom(sources),
			Source::File(path) => {
				let (line, column) = compute_location(data.as_bytes(), whole_slice.as_bytes());
				SourceLocation::File(FileLocation { path, line, column })
			}
		})
	}
}

impl<'a> FileTracker<[u8]> for SliceTracker<'a, [u8], Source<'a, [u8]>> {
	fn insert_file(&self, path: impl Into<PathBuf>) -> std::io::Result<&[u8]> {
		let path = path.into();
		let data = read_binary_file(&path)?;
		if data.is_empty() {
			Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "file is empty"))
		} else {
			// New strings can't be in the tracker yet, so this should be safe.
			Ok(unsafe { self.insert_unsafe(Cow::Owned(data), Source::File(path)) })
		}
	}

	fn get_source_location(&self, data: &[u8]) -> Option<SourceLocation<[u8]>> {
		let (whole_slice, source) = self.get(data)?;
		Some(match source {
			Source::Unknown => SourceLocation::Unknown,
			Source::ExpandedFrom(sources) => SourceLocation::ExpandedFrom(sources),
			Source::File(path) => {
				let (line, column) = compute_location(data, whole_slice);
				SourceLocation::File(FileLocation { path, line, column })
			}
		})
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use assert2::assert;

	#[test]
	fn test_compute_location() {
		let data = b"hello\nworld";

		assert!(compute_location(&data[0..], data) == (1, 1));
		assert!(compute_location(&data[1..], data) == (1, 2));
		assert!(compute_location(&data[2..], data) == (1, 3));
		assert!(compute_location(&data[3..], data) == (1, 4));
		assert!(compute_location(&data[4..], data) == (1, 5));
		assert!(compute_location(&data[5..], data) == (1, 6));
		assert!(compute_location(&data[6..], data) == (2, 1));
		assert!(compute_location(&data[7..], data) == (2, 2));

		let data = b"a\r\na\n";
		assert!(compute_location(&data[0..], data) == (1, 1));
		assert!(compute_location(&data[1..], data) == (1, 2));
		assert!(compute_location(&data[2..], data) == (1, 3));
		assert!(compute_location(&data[3..], data) == (2, 1));
		assert!(compute_location(&data[4..], data) == (2, 2));
	}
}
