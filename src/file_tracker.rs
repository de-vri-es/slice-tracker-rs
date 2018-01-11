use std;
use std::borrow::Cow;
use std::fs::File;
use std::io::Read;
use std::path::{Path,PathBuf};

use super::SliceTracker;
use super::SourceLocation;

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
