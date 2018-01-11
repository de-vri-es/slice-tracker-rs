use std;
use std::borrow::Cow;
use std::collections::Bound::{Excluded, Included, Unbounded};
use std::collections::btree_map::Entry as BTreeMapEntry;
use std::fs::File;
use std::io::Read;
use std::path::{Path,PathBuf};

use super::slice::Slice;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd)]
pub enum Source<'a, 'path, B> where B: 'a + ?Sized {
	Other,
	ExpandedFrom(&'a B),
	File(&'path Path),
}

enum SourceStorage<'a, B> where B: 'a + ?Sized {
	Other,
	ExpandedFrom(&'a B),
	File(PathBuf),
}

pub struct Entry<'a, B> where B: 'a + ?Sized + ToOwned {
	data: Cow<'a, B>,
	source: SourceStorage<'a, B>,
}

impl<'a, 'path, B> Source<'a, 'path, B> where B: ?Sized {
	fn to_storage(self) -> SourceStorage<'a, B> {
		match self {
			Source::Other                => SourceStorage::Other,
			Source::ExpandedFrom(string) => SourceStorage::ExpandedFrom(string),
			Source::File(path)           => SourceStorage::File(path.to_owned()),
		}
	}
}

impl<'a, B> SourceStorage<'a, B> where B: ?Sized {
	fn to_source<'b>(&'b self) -> Source<'a, 'b, B> {
		match self {
			&SourceStorage::Other                    => Source::Other,
			&SourceStorage::ExpandedFrom(ref string) => Source::ExpandedFrom(string),
			&SourceStorage::File(ref path)           => Source::File(path),
		}
	}
}

/// Read a file into a string.
fn read_text_file<P: ?Sized + AsRef<Path>>(path: &P) -> std::io::Result<String> {
	let mut file = File::open(path)?;
	let mut data = String::new();
	file.read_to_string(&mut data)?;
	return Ok(data);
}

/// Tracker for strings with metadata.
///
/// The tracker can take ownership or store references if their lifetime is long enough.
/// Each string added to the tracker has some source information attached to it.
/// This information can later be retrieved from the tracker with a (partial) &str.
///
/// The tracker can not track empty strings,
/// and it can not look up information for empty string slices.
pub struct StringTracker<'a, B> where B: 'a + ?Sized + ToOwned + Slice {
	map: std::cell::UnsafeCell<std::collections::BTreeMap<*const B::PtrType, Entry<'a, B>>>
}

impl<'a, B> StringTracker<'a, B> where B: 'a + ?Sized + ToOwned + Slice {
	/// Create a new string tracker.
	pub fn new() -> Self {
		StringTracker{map: std::cell::UnsafeCell::new(std::collections::BTreeMap::new())}
	}

	/// Insert a borrowed reference in the tracker.
	///
	/// Fails if the string is empty or if it is already tracked.
	pub fn insert_borrow<'path, S: ?Sized + AsRef<B>>(&self, data: &'a S, source: Source<'a, 'path, B>) -> Result<&B, ()> {
		Ok(self.insert_with_source(Cow::Borrowed(data.as_ref()), source)?)
	}

	/// Move a string into the tracker.
	///
	/// Fails if the string is empty.
	pub fn insert_move<'path, S: Into<B::Owned>>(&self, data: S, source: Source<'a, 'path, B>) -> Result<&B, ()> {
		// New string can't be in the map yet, but empty string can not be inserted.
		Ok(self.insert_with_source(Cow::Owned(data.into()), source)?)
	}

	/// Check if a string slice is tracked.
	pub fn is_tracked(&self, data: &B) -> bool {
		self.get_entry(data).is_some()
	}

	/// Get the whole tracked slice and source information for a string slice.
	pub fn get(&self, data: &B) -> Option<(&B, Source<B>)> {
		self.get_entry(data).map(|entry| {
			(entry.data.as_ref(), entry.source.to_source())
		})
	}

	/// Get the source information for a string slice.
	pub fn get_source(&self, data: &B) -> Option<Source<B>> {
		self.get_entry(data).map(|entry| entry.source.to_source())
	}

	/// Get the whole tracked slice for a string slice.
	pub fn get_whole_slice(&self, data: &B) -> Option<&B> {
		self.get_entry(data).map(|entry| entry.data.as_ref())
	}

// private:

	/// Get the map from the UnsafeCell.
	fn map(&self) -> &std::collections::BTreeMap<*const B::PtrType, Entry<'a, B>> {
		unsafe { &*self.map.get() }
	}

	/// Get the map from the UnsafeCell as mutable map.
	fn map_mut(&self) -> &mut std::collections::BTreeMap<*const B::PtrType, Entry<'a, B>> {
		unsafe { &mut *self.map.get() }
	}

	/// Find the first entry with start_ptr <= the given bound.
	fn first_entry_at_or_before(&self, bound: *const B::PtrType) -> Option<&Entry<B>> {
		let (_key, value) = self.map().range((Unbounded, Included(bound))).next_back()?;
		Some(&value)
	}

	/// Find the first entry with start_ptr < the given bound.
	fn first_entry_before(&self, bound: *const B::PtrType) -> Option<&Entry<B>> {
		let (_key, value) = self.map().range((Unbounded, Excluded(bound))).next_back()?;
		Some(&value)
	}

	/// Get the entry tracking a string.
	fn get_entry(&self, data: &B) -> Option<&Entry<B>> {
		// Empty strings aren't tracked.
		// They can't be distuingished from str_a[end..end] or str_b[0..0],
		// if str_a and str_b directly follow eachother in memory.
		if data.is_empty() { return None }

		// Get the last element where start_ptr <= data.start_ptr
		let entry = self.first_entry_at_or_before(data.start_ptr())?;
		if data.end_ptr() <= entry.data.end_ptr() {
			Some(entry)
		} else {
			None
		}
	}

	/// Check if the given data has overlap with anything in the string tracker.
	fn has_overlap<S: ?Sized + AsRef<B>>(&self, data: &S) -> bool {
		let data = data.as_ref();

		// Empty slices can't overlap with anything, even if their start pointer is tracked.
		if data.is_empty() { return false }

		// Last element with start < data.end_ptr()
		let conflict = match self.first_entry_before(data.end_ptr()) {
			None        => return false,
			Some(entry) => entry,
		};

		// If conflict doesn't end before data starts, it's a conflict.
		// Though end is one-past the end, so end == start is also okay.
		conflict.data.end_ptr() > data.start_ptr()
	}

	/// Insert data with source information without checking if the data is already present.
	unsafe fn insert_unsafe<'path>(&self, data: Cow<'a, B>, source: SourceStorage<'a, B>) -> &B {
		// Insert the data itself.
		match self.map_mut().entry(data.start_ptr()) {
			BTreeMapEntry::Vacant(x)   => x.insert(Entry{data, source}).data.as_ref(),
			BTreeMapEntry::Occupied(_) => unreachable!(),
		}
	}

	/// Like insert, but convert the Source to SourceStorage only after all checks are done.
	fn insert_with_source<'path>(&self, data: Cow<'a, B>, source: Source<'a, 'path, B>) -> Result<&B, ()> {
		// Reject empty data or data that is already (partially) tracked.
		if data.is_empty() || self.has_overlap(&data) { return Err(()) }
		Ok(unsafe { self.insert_unsafe(data, source.to_storage()) })
	}
}

impl<'a> StringTracker<'a, str> {
	/// Read a file and insert it into the tracker.
	///
	/// Fails if reading the file fails, or if the file is empty.
	pub fn insert_file<P: Into<PathBuf>>(&self, path: P) -> std::io::Result<&str> {
		let path = path.into();
		let data = read_text_file(&path)?;
		if data.is_empty() {
			Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "file is empty"))
		} else {
			Ok(unsafe { self.insert_unsafe(Cow::Owned(data), SourceStorage::File(path)) })
		}
	}
}

impl<'a, B> Default for StringTracker<'a, B> where B: ?Sized + ToOwned + Slice {
	fn default() -> Self { Self::new() }
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn test_insert_borrow() {
		let pool = StringTracker::default();
		let data = "aap noot mies";
		let len  = data.len();
		assert_eq!(pool.is_tracked(data), false);

		// Cant insert empty string slices.
		assert!(pool.insert_borrow("",          Source::Other).is_err());
		assert!(pool.insert_borrow(&data[3..3], Source::Other).is_err());

		// Can insert non-empty str only once.
		let tracked = pool.insert_borrow(data, Source::Other).unwrap();
		assert!(pool.insert_borrow(data, Source::Other).is_err());
		assert!(pool.is_tracked(data));

		// is_tracked says no to empty sub-slices.
		assert!(!pool.is_tracked(&data[0..0]));
		assert!(!pool.is_tracked(&data[1..1]));
		assert!(!pool.is_tracked(&data[len..len]));

		// non-empty sub-slices give the whole slice back.
		assert!(std::ptr::eq(data, tracked));
		assert!(std::ptr::eq(data, pool.get_whole_slice(data).unwrap()));
		assert!(std::ptr::eq(data, pool.get_whole_slice(&data[0..1]).unwrap()));
		assert!(std::ptr::eq(data, pool.get_whole_slice(&data[4..8]).unwrap()));
		assert!(std::ptr::eq(data, pool.get_whole_slice(&data[len-1..len]).unwrap()));
		assert!(std::ptr::eq(data, pool.get_whole_slice(&data[..]).unwrap()));
	}

	#[test]
	fn test_insert_part() {
		let pool = StringTracker::default();
		let data = "aap noot mies";
		let noot = &data[4..8];
		assert_eq!(noot, "noot");


		// Adding the subslice to the pool doesn't make the whole str tracked.
		let tracked = pool.insert_borrow(noot, Source::Other).unwrap();
		assert!(pool.is_tracked(noot));
		assert!(pool.is_tracked(&data[4..8]));
		assert!(!pool.is_tracked(data));
		assert!(!pool.is_tracked(&data[ ..4]));
		assert!(!pool.is_tracked(&data[8.. ]));

		// But we can't track the whole slice anymore now.
		assert!(pool.insert_borrow(data, Source::Other).is_err());

		// Subslices from the original str in the right range give the whole tracked subslice.
		assert!(std::ptr::eq(noot, tracked));
		assert!(std::ptr::eq(noot, pool.get_whole_slice(noot).unwrap()));
		assert!(std::ptr::eq(noot, pool.get_whole_slice(&data[4..8]).unwrap()));
		assert!(std::ptr::eq(noot, pool.get_whole_slice(&data[4..7]).unwrap()));
		assert!(std::ptr::eq(noot, pool.get_whole_slice(&data[5..8]).unwrap()));
		assert!(std::ptr::eq(noot, pool.get_whole_slice(&data[5..7]).unwrap()));
	}

	#[test]
	fn test_insert_move() {
		let pool = StringTracker::default();

		// Can't insert empty strings.
		assert!(pool.insert_move("",            Source::Other).is_err());
		assert!(pool.insert_move(String::new(), Source::Other).is_err());

		let data: &str = pool.insert_move("aap noot mies", Source::Other).unwrap();
		let len = data.len();
		assert!(pool.is_tracked(data), true);
		assert!(!pool.is_tracked(&data[0..0]));
		assert!(!pool.is_tracked(&data[5..5]));
		assert!(!pool.is_tracked(&data[len..len]));
		assert!(!pool.is_tracked("aap"));

		assert!(std::ptr::eq(data, pool.get_whole_slice(data).unwrap()));
		assert!(std::ptr::eq(data, pool.get_whole_slice(&data[0..1]).unwrap()));
		assert!(std::ptr::eq(data, pool.get_whole_slice(&data[4..8]).unwrap()));
		assert!(std::ptr::eq(data, pool.get_whole_slice(&data[len-1..len]).unwrap()));
		assert!(std::ptr::eq(data, pool.get_whole_slice(&data[..]).unwrap()));
	}
}
