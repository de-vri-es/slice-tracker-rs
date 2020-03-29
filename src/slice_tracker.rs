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
use std::collections::btree_map::Entry as BTreeMapEntry;
use std::collections::Bound::{Excluded, Included, Unbounded};

use super::Slice;
use super::StableBorrow;

pub struct Entry<'a, B, M>
where
	B: 'a + ?Sized + ToOwned,
	M: ?Sized,
{
	data: Cow<'a, B>,
	meta: Box<M>,
}

/// Tracker for slices with metadata.
///
/// The tracker can take ownership or store references if their lifetime is long enough.
/// Each slice added to the tracker has some metadata attached to it.
/// This information can later be retrieved from the tracker with a subslice of the tracked slice.
///
/// The tracker can not track empty slices, and it can not look up information for empty slices.
pub struct SliceTracker<'a, B, M>
where
	B: 'a + ?Sized + ToOwned + Slice,
	B::Owned: StableBorrow,
{
	map: std::cell::UnsafeCell<std::collections::BTreeMap<*const B::PtrType, Entry<'a, B, M>>>,
}

impl<'a, B, M> SliceTracker<'a, B, M>
where
	B: 'a + ?Sized + ToOwned + Slice,
	B::Owned: StableBorrow,
{
	/// Create a new slice tracker.
	pub fn new() -> Self {
		SliceTracker {
			map: std::cell::UnsafeCell::new(std::collections::BTreeMap::new()),
		}
	}

	/// Insert a slice with metadata without checking if the data is already present.
	pub unsafe fn insert_unsafe<'path>(&self, data: Cow<'a, B>, meta: impl Into<Box<M>>) -> &B {
		// Insert the data itself.
		match self.map_mut().entry(data.start_ptr()) {
			BTreeMapEntry::Vacant(x) => x
				.insert(Entry {
					data,
					meta: meta.into(),
				})
				.data
				.as_ref(),
			BTreeMapEntry::Occupied(_) => unreachable!(),
		}
	}

	/// Safely insert a slice with metadata.
	pub fn insert<'path>(&self, data: Cow<'a, B>, meta: impl Into<Box<M>>) -> Result<&B, ()> {
		// Reject empty data or data that is already (partially) tracked.
		if data.is_empty() || self.has_overlap(&data) {
			return Err(());
		}
		Ok(unsafe { self.insert_unsafe(data, meta) })
	}

	/// Insert a borrowed reference in the tracker.
	///
	/// Fails if the slice is empty or if (parts of) it are already tracked.
	pub fn insert_borrow<'path, S: ?Sized + AsRef<B>>(&self, data: &'a S, meta: impl Into<Box<M>>) -> Result<&B, ()> {
		self.insert(Cow::Borrowed(data.as_ref()), meta)
	}

	/// Move an owned slice into the tracker.
	/// The tracker takes ownership of the data.
	///
	/// Fails if the slice is empty.
	pub fn insert_move<'path, S: Into<B::Owned>>(&self, data: S, meta: impl Into<Box<M>>) -> Result<&B, ()> {
		// New owned slices can't be in the map yet, but empty slices can't be inserted.
		self.insert(Cow::Owned(data.into()), meta)
	}

	/// Check if a slice is tracked.
	pub fn is_tracked(&self, data: &B) -> bool {
		self.get_entry(data).is_some()
	}

	/// Get the whole tracked slice and metadata for a (partial) slice.
	pub fn get(&self, data: &B) -> Option<(&B, &M)> {
		self.get_entry(data)
			.map(|entry| (entry.data.as_ref(), entry.meta.as_ref()))
	}

	/// Get the metadata for a (partial) slice.
	pub fn metadata(&self, data: &B) -> Option<&M> {
		self.get_entry(data).map(|entry| entry.meta.as_ref())
	}

	/// Get the whole tracked slice for a (partial) slice.
	pub fn whole_slice(&self, data: &B) -> Option<&B> {
		self.get_entry(data).map(|entry| entry.data.as_ref())
	}

	// private:

	/// Get the map from the UnsafeCell.
	fn map(&self) -> &std::collections::BTreeMap<*const B::PtrType, Entry<'a, B, M>> {
		unsafe { &*self.map.get() }
	}

	/// Get the map from the UnsafeCell as mutable map.
	fn map_mut(&self) -> &mut std::collections::BTreeMap<*const B::PtrType, Entry<'a, B, M>> {
		unsafe { &mut *self.map.get() }
	}

	/// Find the first entry with start_ptr <= the given bound.
	fn first_entry_at_or_before(&self, bound: *const B::PtrType) -> Option<&Entry<B, M>> {
		let (_key, value) = self.map().range((Unbounded, Included(bound))).next_back()?;
		Some(&value)
	}

	/// Find the first entry with start_ptr < the given bound.
	fn first_entry_before(&self, bound: *const B::PtrType) -> Option<&Entry<B, M>> {
		let (_key, value) = self.map().range((Unbounded, Excluded(bound))).next_back()?;
		Some(&value)
	}

	/// Get the tracking entry for a slice.
	fn get_entry(&self, data: &B) -> Option<&Entry<B, M>> {
		// Empty slices can not be tracked.
		// They can't be distuingished from str_a[end..end] or str_b[0..0],
		// if str_a and str_b directly follow eachother in memory.
		if data.is_empty() {
			return None;
		}

		// Get the last element where start_ptr <= data.start_ptr
		let entry = self.first_entry_at_or_before(data.start_ptr())?;
		if data.end_ptr() <= entry.data.end_ptr() {
			Some(entry)
		} else {
			None
		}
	}

	/// Check if the given slice has overlap with anything in the slice tracker.
	fn has_overlap<S: ?Sized + AsRef<B>>(&self, data: &S) -> bool {
		let data = data.as_ref();

		// Empty slices can't overlap with anything, even if their start pointer is tracked.
		if data.is_empty() {
			return false;
		}

		// Last element with start < data.end_ptr()
		let conflict = match self.first_entry_before(data.end_ptr()) {
			None => return false,
			Some(entry) => entry,
		};

		// If conflict doesn't end before data starts, it's a conflict.
		// Though end is one-past the end, so end == start is also okay.
		conflict.data.end_ptr() > data.start_ptr()
	}
}

impl<'a, B, M> Default for SliceTracker<'a, B, M>
where
	B: 'a + ?Sized + ToOwned + Slice,
	B::Owned: StableBorrow,
{
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn test_insert_borrow() {
		let pool = SliceTracker::<str, ()>::default();
		let data = "aap noot mies";
		let len = data.len();
		assert_eq!(pool.is_tracked(data), false);

		// Cant insert empty string slices.
		assert!(pool.insert_borrow("", ()).is_err());
		assert!(pool.insert_borrow(&data[3..3], ()).is_err());

		// Can insert non-empty str only once.
		let tracked = pool.insert_borrow(data, ()).unwrap();
		assert!(pool.insert_borrow(data, ()).is_err());
		assert!(pool.is_tracked(data));

		// is_tracked says no to empty sub-slices.
		assert!(!pool.is_tracked(&data[0..0]));
		assert!(!pool.is_tracked(&data[1..1]));
		assert!(!pool.is_tracked(&data[len..len]));

		// non-empty sub-slices give the whole slice back.
		assert!(std::ptr::eq(data, tracked));
		assert!(std::ptr::eq(data, pool.whole_slice(data).unwrap()));
		assert!(std::ptr::eq(data, pool.whole_slice(&data[0..1]).unwrap()));
		assert!(std::ptr::eq(data, pool.whole_slice(&data[4..8]).unwrap()));
		assert!(std::ptr::eq(data, pool.whole_slice(&data[len - 1..len]).unwrap()));
		assert!(std::ptr::eq(data, pool.whole_slice(&data[..]).unwrap()));
	}

	#[test]
	fn test_insert_part() {
		let pool = SliceTracker::<str, ()>::default();
		let data = "aap noot mies";
		let noot = &data[4..8];
		assert_eq!(noot, "noot");

		// Adding the subslice to the pool doesn't make the whole str tracked.
		let tracked = pool.insert_borrow(noot, ()).unwrap();
		assert!(pool.is_tracked(noot));
		assert!(pool.is_tracked(&data[4..8]));
		assert!(!pool.is_tracked(data));
		assert!(!pool.is_tracked(&data[..4]));
		assert!(!pool.is_tracked(&data[8..]));

		// But we can't track the whole slice anymore now.
		assert!(pool.insert_borrow(data, ()).is_err());

		// Subslices from the original str in the right range give the whole tracked subslice.
		assert!(std::ptr::eq(noot, tracked));
		assert!(std::ptr::eq(noot, pool.whole_slice(noot).unwrap()));
		assert!(std::ptr::eq(noot, pool.whole_slice(&data[4..8]).unwrap()));
		assert!(std::ptr::eq(noot, pool.whole_slice(&data[4..7]).unwrap()));
		assert!(std::ptr::eq(noot, pool.whole_slice(&data[5..8]).unwrap()));
		assert!(std::ptr::eq(noot, pool.whole_slice(&data[5..7]).unwrap()));
	}

	#[test]
	fn test_insert_move() {
		let pool = SliceTracker::<str, ()>::default();

		// Can't insert empty strings.
		assert!(pool.insert_move("", ()).is_err());
		assert!(pool.insert_move(String::new(), ()).is_err());

		let data: &str = pool.insert_move("aap noot mies", ()).unwrap();
		let len = data.len();
		assert!(pool.is_tracked(data), true);
		assert!(!pool.is_tracked(&data[0..0]));
		assert!(!pool.is_tracked(&data[5..5]));
		assert!(!pool.is_tracked(&data[len..len]));
		assert!(!pool.is_tracked("aap"));

		assert!(std::ptr::eq(data, pool.whole_slice(data).unwrap()));
		assert!(std::ptr::eq(data, pool.whole_slice(&data[0..1]).unwrap()));
		assert!(std::ptr::eq(data, pool.whole_slice(&data[4..8]).unwrap()));
		assert!(std::ptr::eq(data, pool.whole_slice(&data[len - 1..len]).unwrap()));
		assert!(std::ptr::eq(data, pool.whole_slice(&data[..]).unwrap()));
	}
}
