use std::cmp::Ordering;

use crate::{
	utils::{binary_search_min, Array},
	Storage, M,
};

use super::{Balance, Offset, WouldUnderflow};

#[derive(Clone)]
pub struct Leaf<T, S: Storage<T>> {
	parent: Option<S::Node>,
	items: Array<T, { M + 1 }>,
}

impl<T, S: Storage<T>> Leaf<T, S> {
	#[inline]
	pub fn new(parent: Option<S::Node>, item: T) -> Leaf<T, S> {
		let mut items = Array::new();
		items.push(item);

		Leaf { parent, items }
	}

	/// Forget the node content without running the items destructors.
	pub fn forget(&mut self) {
		std::mem::forget(std::mem::take(&mut self.items));
	}

	#[inline]
	pub fn parent(&self) -> Option<S::Node> {
		self.parent
	}

	#[inline]
	pub fn set_parent(&mut self, p: Option<S::Node>) {
		self.parent = p
	}

	#[inline]
	pub fn item_count(&self) -> usize {
		self.items.len()
	}

	#[inline]
	pub fn items(&self) -> &[T] {
		self.items.as_ref()
	}

	#[inline]
	pub fn iter(&self) -> std::slice::Iter<T> {
		self.items.as_ref().iter()
	}

	#[inline]
	pub fn get<Q: ?Sized>(&self, cmp: impl Fn(&T, &Q) -> Ordering, key: &Q) -> Option<&T> {
		match binary_search_min(cmp, &self.items, key) {
			Some((i, eq)) => {
				if eq {
					Some(&self.items[i])
				} else {
					None
				}
			}
			_ => None,
		}
	}

	#[inline]
	pub fn get_mut<Q: ?Sized>(
		&mut self,
		cmp: impl Fn(&T, &Q) -> Ordering,
		key: &Q,
	) -> Option<&mut T> {
		match binary_search_min(cmp, &self.items, key) {
			Some((i, eq)) => {
				if eq {
					Some(&mut self.items[i])
				} else {
					None
				}
			}
			_ => None,
		}
	}

	/// Find the offset of the item matching the given key.
	#[inline]
	pub fn offset_of<Q: ?Sized>(
		&self,
		cmp: impl Fn(&T, &Q) -> Ordering,
		key: &Q,
	) -> Result<Offset, Offset> {
		match binary_search_min(cmp, &self.items, key) {
			Some((i, eq)) => {
				if eq {
					Ok(i.into())
				} else {
					Err((i + 1).into())
				}
			}
			None => Err(0.into()),
		}
	}

	#[inline]
	pub fn item(&self, offset: Offset) -> Option<&T> {
		match offset.value() {
			Some(offset) => self.items.get(offset),
			None => None,
		}
	}

	#[inline]
	pub fn item_mut(&mut self, offset: Offset) -> Option<&mut T> {
		match offset.value() {
			Some(offset) => self.items.get_mut(offset),
			None => None,
		}
	}

	#[inline]
	pub fn insert_by_key(
		&mut self,
		cmp: impl Fn(&T, &T) -> Ordering,
		mut item: T,
	) -> (Offset, Option<T>) {
		match binary_search_min(cmp, &self.items, &item) {
			Some((i, eq)) => {
				if eq {
					std::mem::swap(&mut item, &mut self.items[i]);
					(i.into(), Some(item))
				} else {
					self.items.insert(i + 1, item);
					((i + 1).into(), None)
				}
			}
			None => {
				self.items.insert(0, item);
				(0.into(), None)
			}
		}
	}

	#[inline]
	pub fn split(&mut self) -> (usize, T, Leaf<T, S>) {
		assert!(self.is_overflowing());

		let median_i = (self.items.len() - 1) / 2;

		let right_items = self.items.drain(median_i + 1..).collect();
		let median = self.items.pop().unwrap();

		let right_leaf = Leaf {
			parent: self.parent,
			items: right_items,
		};

		assert!(!self.is_underflowing());
		assert!(!right_leaf.is_underflowing());

		(self.items.len(), median, right_leaf)
	}

	#[inline]
	pub fn append(&mut self, separator: T, mut other: Leaf<T, S>) -> Offset {
		let offset = self.items.len();
		self.items.push(separator);
		self.items.append(&mut other.items);
		offset.into()
	}

	#[inline]
	pub fn push_left(&mut self, item: T) {
		self.items.insert(0, item)
	}

	#[inline]
	pub fn pop_left(&mut self) -> Result<T, WouldUnderflow> {
		if self.item_count() < M / 2 {
			Err(WouldUnderflow)
		} else {
			Ok(self.items.remove(0).unwrap())
		}
	}

	#[inline]
	pub fn push_right(&mut self, item: T) -> Offset {
		let offset = self.items.len();
		self.items.push(item);
		offset.into()
	}

	#[inline]
	pub fn pop_right(&mut self) -> Result<(Offset, T), WouldUnderflow> {
		if self.item_count() < M / 2 {
			Err(WouldUnderflow)
		} else {
			let offset = self.items.len();
			let item = self.items.pop().unwrap();
			Ok((offset.into(), item))
		}
	}

	#[inline]
	pub fn balance(&self) -> Balance {
		if self.is_overflowing() {
			Balance::Overflow
		} else if self.is_underflowing() {
			Balance::Underflow(self.items.is_empty())
		} else {
			Balance::Balanced
		}
	}

	#[inline]
	pub fn is_overflowing(&self) -> bool {
		self.item_count() > M
	}

	#[inline]
	pub fn is_underflowing(&self) -> bool {
		self.item_count() < M / 2 - 1
	}

	/// It is assumed that the leaf will not overflow.
	#[inline]
	pub fn insert(&mut self, offset: Offset, item: T) {
		match offset.value() {
			Some(offset) => self.items.insert(offset, item),
			None => panic!("Offset out of bounds"),
		}
	}

	/// Remove the item at the given offset.
	/// Return the new balance of the leaf.
	#[inline]
	pub fn remove(&mut self, offset: Offset) -> T {
		match offset.value() {
			Some(offset) => self.items.remove(offset).unwrap(),
			None => panic!("Offset out of bounds"),
		}
	}

	#[inline]
	pub fn remove_last(&mut self) -> T {
		self.items.pop().unwrap()
	}

	/// Write the label of the leaf in the DOT language.
	///
	/// Requires the `dot` feature.
	#[cfg(feature = "dot")]
	#[inline]
	pub fn dot_write_label<W: std::io::Write>(&self, f: &mut W) -> std::io::Result<()>
	where
		T: std::fmt::Display,
	{
		for item in &self.items {
			write!(f, "{{{}}}|", item)?;
		}

		Ok(())
	}

	#[cfg(debug_assertions)]
	pub fn validate(
		&self,
		cmp: impl Fn(&T, &T) -> Ordering,
		parent: Option<S::Node>,
		min: Option<&T>,
		max: Option<&T>,
	) {
		if self.parent() != parent {
			panic!("wrong parent")
		}

		if min.is_some() || max.is_some() {
			// not root
			match self.balance() {
				Balance::Overflow => panic!("leaf is overflowing"),
				Balance::Underflow(_) => panic!("leaf is underflowing"),
				_ => (),
			}
		}

		if !self.items.windows(2).all(|w| cmp(&w[0], &w[1]).is_lt()) {
			panic!("leaf items are not sorted")
		}

		if let Some(min) = min {
			if let Some(item) = self.items.first() {
				if cmp(min, item).is_ge() {
					panic!("leaf item key is greater than right separator")
				}
			}
		}

		if let Some(max) = max {
			if let Some(item) = self.items.last() {
				if cmp(max, item).is_le() {
					panic!("leaf item key is less than left separator")
				}
			}
		}
	}
}
