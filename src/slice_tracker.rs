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

use std::cell::UnsafeCell;
use std::collections::btree_map;
use std::collections::BTreeMap;
use std::collections::Bound::{Excluded, Included, Unbounded};

use super::BorrowSlice;
use super::Slice;
use super::StableBorrow;

pub struct Entry<Data, Metadata> {
	/// The data being tracked.
	data: Data,

	/// Metadata for the entry.
	///
	/// The metadata is kept in a box so references remain valid
	/// when new entries are added to the map.
	meta: Box<Metadata>,
}

/// Tracker for slices with metadata.
///
/// The tracker can take ownership or store references if their lifetime is long enough.
/// Each slice added to the tracker has some metadata attached to it.
/// This information can later be retrieved from the tracker with a subslice of the tracked slice.
///
/// The tracker can not track empty slices, and it can not look up information for empty slices.
pub struct SliceTracker<Data, Metadata>
where
	Data: BorrowSlice + StableBorrow,
{
	map: UnsafeCell<BTreeMap<*const <Data::Slice as Slice>::Element, Entry<Data, Metadata>>>,
}

impl<Data, Metadata> Default for SliceTracker<Data, Metadata>
where
	Data: BorrowSlice + StableBorrow,
{
	fn default() -> Self {
		Self::new()
	}
}

impl<Data, Metadata> SliceTracker<Data, Metadata>
where
	Data: BorrowSlice + StableBorrow,
{
	/// Create a new slice tracker.
	pub fn new() -> Self {
		Self {
			map: UnsafeCell::new(BTreeMap::new()),
		}
	}

	/// Insert a slice with metadata without checking if the data is already present.
	pub unsafe fn insert_unsafe<'path>(&self, data: Data, meta: impl Into<Box<Metadata>>) -> &Data::Slice {
		// Insert the data itself.
		match self.map_mut().entry(data.start_ptr()) {
			btree_map::Entry::Vacant(x) => &x
				.insert(Entry {
					data,
					meta: meta.into(),
				})
				.data
				.borrow_slice(),
			btree_map::Entry::Occupied(_) => unreachable!(),
		}
	}

	/// Safely insert a slice with metadata.
	pub fn insert<'path>(&self, data: Data, meta: impl Into<Box<Metadata>>) -> Result<&Data::Slice, ()> {
		// Reject empty data or data that is already (partially) tracked.
		if data.is_empty() || self.has_overlap(data.borrow_slice()) {
			return Err(());
		}
		Ok(unsafe { self.insert_unsafe(data, meta) })
	}

	/// Check if a slice is tracked.
	pub fn is_tracked(&self, data: &Data::Slice) -> bool {
		self.get_entry(data).is_some()
	}

	/// Get the whole tracked slice and metadata for a (partial) slice.
	pub fn get(&self, data: &Data::Slice) -> Option<(&Data::Slice, &Metadata)> {
		self.get_entry(data)
			.map(|entry| (entry.data.borrow_slice(), entry.meta.as_ref()))
	}

	/// Get the metadata for a (partial) slice.
	pub fn metadata(&self, data: &Data::Slice) -> Option<&Metadata> {
		self.get_entry(data).map(|entry| entry.meta.as_ref())
	}

	/// Get the whole tracked slice for a (partial) slice.
	pub fn whole_slice(&self, data: &Data::Slice) -> Option<&Data::Slice> {
		self.get_entry(data).map(|entry| entry.data.borrow_slice())
	}

	/// Get the map from the UnsafeCell.
	fn map(&self) -> &BTreeMap<*const <Data::Slice as Slice>::Element, Entry<Data, Metadata>> {
		unsafe { &*self.map.get() }
	}

	/// Get the map from the UnsafeCell as mutable map.
	fn map_mut(&self) -> &mut BTreeMap<*const <Data::Slice as Slice>::Element, Entry<Data, Metadata>> {
		unsafe { &mut *self.map.get() }
	}

	/// Find the last entry with start_ptr <= the given bound.
	fn last_at_or_before(&self, bound: *const <Data::Slice as Slice>::Element) -> Option<&Entry<Data, Metadata>> {
		let (_key, value) = self.map().range((Unbounded, Included(bound))).next_back()?;
		Some(&value)
	}

	/// Find the last entry with start_ptr < the given bound.
	fn last_before(&self, bound: *const <Data::Slice as Slice>::Element) -> Option<&Entry<Data, Metadata>> {
		let (_key, value) = self.map().range((Unbounded, Excluded(bound))).next_back()?;
		Some(&value)
	}

	/// Get the tracking entry for a slice.
	fn get_entry(&self, data: &Data::Slice) -> Option<&Entry<Data, Metadata>> {
		// Empty slices can not be tracked.
		// They can't be distuingished from str_a[end..end] or str_b[0..0],
		// if str_a and str_b directly follow eachother in memory.
		if data.is_empty() {
			return None;
		}

		// Get the last element where start_ptr <= data.start_ptr
		let entry = self.last_at_or_before(data.start_ptr())?;
		if data.end_ptr() <= entry.data.borrow_slice().end_ptr() {
			Some(entry)
		} else {
			None
		}
	}

	/// Check if the given slice has overlap with anything in the slice tracker.
	fn has_overlap(&self, data: &Data::Slice) -> bool {
		// Empty slices can't overlap with anything, even if their start pointer is tracked.
		if data.is_empty() {
			return false;
		}

		// Last element with start < data.end_ptr()
		let conflict = match self.last_before(data.end_ptr()) {
			None => return false,
			Some(entry) => entry,
		};

		// If conflict doesn't end before data starts, it's a conflict.
		// Though end is one-past the end, so end == start is also okay.
		conflict.data.borrow_slice().end_ptr() > data.start_ptr()
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn test_insert() {
		let pool = SliceTracker::<&str, ()>::default();
		let data = "aap noot mies";
		let len = data.len();
		assert_eq!(pool.is_tracked(data), false);

		// Cant insert empty string slices.
		assert!(pool.insert("", ()).is_err());
		assert!(pool.insert(&data[3..3], ()).is_err());

		// Can insert non-empty str only once.
		let tracked = pool.insert(data, ()).unwrap();
		assert!(pool.insert(data, ()).is_err());
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
		let pool = SliceTracker::<&str, ()>::default();
		let data = "aap noot mies";
		let noot = &data[4..8];
		assert_eq!(noot, "noot");

		// Adding the subslice to the pool doesn't make the whole str tracked.
		let tracked = pool.insert(noot, ()).unwrap();
		assert!(pool.is_tracked(noot));
		assert!(pool.is_tracked(&data[4..8]));
		assert!(!pool.is_tracked(data));
		assert!(!pool.is_tracked(&data[..4]));
		assert!(!pool.is_tracked(&data[8..]));

		// But we can't track the whole slice anymore now.
		assert!(pool.insert(data, ()).is_err());

		// Subslices from the original str in the right range give the whole tracked subslice.
		assert!(std::ptr::eq(noot, tracked));
		assert!(std::ptr::eq(noot, pool.whole_slice(noot).unwrap()));
		assert!(std::ptr::eq(noot, pool.whole_slice(&data[4..8]).unwrap()));
		assert!(std::ptr::eq(noot, pool.whole_slice(&data[4..7]).unwrap()));
		assert!(std::ptr::eq(noot, pool.whole_slice(&data[5..8]).unwrap()));
		assert!(std::ptr::eq(noot, pool.whole_slice(&data[5..7]).unwrap()));
	}
}
