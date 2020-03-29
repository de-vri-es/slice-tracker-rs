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
use std::path::{Path, PathBuf};

use crate::SliceTracker;
use crate::SourceLocation;

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

pub trait FileSliceTracker<B: ?Sized> {
	/// Read a file and insert it into the tracker.
	///
	/// Fails if reading the file fails, or if the file is empty.
	fn insert_file(&self, path: impl Into<PathBuf>) -> std::io::Result<&B>;
}

impl<'a> FileSliceTracker<str> for SliceTracker<'a, str, SourceLocation<'a, str>> {
	fn insert_file(&self, path: impl Into<PathBuf>) -> std::io::Result<&str> {
		let path = path.into();
		let data = read_text_file(&path)?;
		if data.is_empty() {
			Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "file is empty"))
		} else {
			// New strings can't be in the tracker yet, so this should be safe.
			Ok(unsafe { self.insert_unsafe(Cow::Owned(data), SourceLocation::File(path)) })
		}
	}
}

impl<'a> FileSliceTracker<[u8]> for SliceTracker<'a, [u8], SourceLocation<'a, [u8]>> {
	fn insert_file(&self, path: impl Into<PathBuf>) -> std::io::Result<&[u8]> {
		let path = path.into();
		let data = read_binary_file(&path)?;
		if data.is_empty() {
			Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "file is empty"))
		} else {
			// New strings can't be in the tracker yet, so this should be safe.
			Ok(unsafe { self.insert_unsafe(Cow::Owned(data), SourceLocation::File(path)) })
		}
	}
}
