use crate::{
	utils::{binary_search_min, Array},
	Storage, M,
};
use std::cmp::Ordering;

use super::{Balance, Children, ChildrenWithSeparators, Offset, WouldUnderflow};

/// Underflow threshold.
///
/// An internal node is underflowing if it has less items than this constant.
const UNDERFLOW: usize = M / 2 - 1;

/// Internal node branch.
///
/// A branch is an item followed by child node identifier.
#[derive(Clone)]
pub struct Branch<T, S: Storage<T>> {
	/// Item.
	pub item: T,

	/// Following child node identifier.
	pub child: S::Node,
}

impl<T, S: Storage<T>> AsRef<T> for Branch<T, S> {
	fn as_ref(&self) -> &T {
		&self.item
	}
}

// impl<T, S: Storage<T>> Keyed for Branch<T, S> {
// 	type Key = K;

// 	#[inline]
// 	fn key(&self) -> &K {
// 		self.item.key()
// 	}
// }

/// Error returned when a direct insertion by key in the internal node failed.
pub struct InsertionError<T, S: Storage<T>> {
	/// Inserted key.
	pub item: T,

	/// Offset of the child in which the key should be inserted instead.
	pub child_offset: usize,

	/// Id of the child in which the key should be inserted instead.
	pub child_id: S::Node,
}

/// Internal node.
///
/// An internal node is a node where each item is surrounded by edges to child nodes.
// #[derive(Clone)]
pub struct Internal<T, S: Storage<T>> {
	parent: Option<S::Node>,
	first_child: S::Node,
	other_children: Array<Branch<T, S>, M>,
}

impl<T, S: Storage<T>> Internal<T, S> {
	pub fn new(
		parent: Option<S::Node>,
		first_child: S::Node,
		other_children: Array<Branch<T, S>, M>,
	) -> Self {
		Self {
			parent,
			first_child,
			other_children,
		}
	}

	/// Creates a binary node (with a single item and two children).
	#[inline]
	pub fn binary(
		parent: Option<S::Node>,
		left_id: S::Node,
		median: T,
		right_id: S::Node,
	) -> Internal<T, S> {
		let mut other_children = Array::new();
		other_children.push(Branch {
			item: median,
			child: right_id,
		});

		Internal {
			parent,
			first_child: left_id,
			other_children,
		}
	}

	/// Forget the node content, without running the items destructors.
	///
	/// The node's children must be manually dropped.
	pub fn forget(&mut self) {
		std::mem::forget(std::mem::take(&mut self.other_children))
	}

	/// Returns the current balance of the node.
	#[inline]
	pub fn balance(&self) -> Balance {
		if self.is_overflowing() {
			Balance::Overflow
		} else if self.is_underflowing() {
			Balance::Underflow(self.other_children.is_empty())
		} else {
			Balance::Balanced
		}
	}

	#[inline]
	pub fn is_overflowing(&self) -> bool {
		self.item_count() >= M
	}

	#[inline]
	pub fn is_underflowing(&self) -> bool {
		self.item_count() < UNDERFLOW
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
		self.other_children.len()
	}

	#[inline]
	pub fn child_count(&self) -> usize {
		1usize + self.item_count()
	}

	#[inline]
	pub fn first_child_id(&self) -> S::Node {
		self.first_child
	}

	#[inline]
	pub fn branches(&self) -> &[Branch<T, S>] {
		self.other_children.as_ref()
	}

	#[inline]
	pub fn child_index(&self, id: S::Node) -> Option<usize> {
		if self.first_child == id {
			Some(0)
		} else {
			for i in 0..self.other_children.len() {
				if self.other_children[i].child == id {
					return Some(i + 1);
				}
			}

			None
		}
	}

	#[inline]
	pub fn child_id(&self, index: usize) -> S::Node {
		if index == 0 {
			self.first_child
		} else {
			self.other_children[index - 1].child
		}
	}

	#[inline]
	pub fn child_id_opt(&self, index: usize) -> Option<S::Node> {
		if index == 0 {
			Some(self.first_child)
		} else {
			self.other_children.get(index - 1).map(|b| b.child)
		}
	}

	#[inline]
	pub fn separators(&self, index: usize) -> (Option<&T>, Option<&T>) {
		let min = if index > 0 {
			Some(&self.other_children[index - 1].item)
		} else {
			None
		};

		let max = if index < self.other_children.len() {
			Some(&self.other_children[index].item)
		} else {
			None
		};

		(min, max)
	}

	#[inline]
	pub fn get<Q: ?Sized>(&self, cmp: impl Fn(&T, &Q) -> Ordering, key: &Q) -> Result<&T, S::Node> {
		match binary_search_min(|a, b| cmp(&a.item, b), &self.other_children, key) {
			Some((offset, eq)) => {
				let b = &self.other_children[offset];
				if eq {
					Ok(&b.item)
				} else {
					Err(b.child)
				}
			}
			None => Err(self.first_child),
		}
	}

	#[inline]
	pub fn get_mut<Q: ?Sized>(
		&mut self,
		cmp: impl Fn(&T, &Q) -> Ordering,
		key: &Q,
	) -> Result<&mut T, &mut S::Node> {
		match binary_search_min(|a, b| cmp(&a.item, b), &self.other_children, key) {
			Some((offset, eq)) => {
				let b = &mut self.other_children[offset];
				if eq {
					Ok(&mut b.item)
				} else {
					Err(&mut b.child)
				}
			}
			None => Err(&mut self.first_child),
		}
	}

	/// Find the offset of the item matching the given key.
	///
	/// If the key matches no item in this node,
	/// this funtion returns the index and id of the child that may match the key.
	#[inline]
	pub fn offset_of<Q: ?Sized>(
		&self,
		cmp: impl Fn(&T, &Q) -> Ordering,
		key: &Q,
	) -> Result<Offset, (usize, S::Node)> {
		match binary_search_min(|a, b| cmp(&a.item, b), &self.other_children, key) {
			Some((offset, eq)) => {
				if eq {
					Ok(offset.into())
				} else {
					let id = self.other_children[offset].child;
					Err((offset + 1, id))
				}
			}
			None => Err((0, self.first_child)),
		}
	}

	#[inline]
	pub fn children(&self) -> Children<T, S> {
		Children::Internal(Some(self.first_child), self.other_children.as_ref().iter())
	}

	#[inline]
	pub fn children_with_separators(&self) -> ChildrenWithSeparators<T, S> {
		ChildrenWithSeparators::Internal(
			Some(self.first_child),
			None,
			self.other_children.as_ref().iter().peekable(),
		)
	}

	#[inline]
	pub fn item(&self, offset: Offset) -> Option<&T> {
		match self.other_children.get(offset.unwrap()) {
			Some(b) => Some(&b.item),
			None => None,
		}
	}

	#[inline]
	pub fn item_mut(&mut self, offset: Offset) -> Option<&mut T> {
		match self.other_children.get_mut(offset.unwrap()) {
			Some(b) => Some(&mut b.item),
			None => None,
		}
	}

	/// Insert by key.
	#[inline]
	pub fn insert_by_key(
		&mut self,
		cmp: impl Fn(&T, &T) -> Ordering,
		mut item: T,
	) -> Result<(Offset, T), InsertionError<T, S>> {
		match binary_search_min(|a, b| cmp(&a.item, b), &self.other_children, &item) {
			Some((i, eq)) => {
				if eq {
					std::mem::swap(&mut item, &mut self.other_children[i].item);
					Ok((i.into(), item))
				} else {
					Err(InsertionError {
						item,
						child_offset: i + 1,
						child_id: self.other_children[i].child,
					})
				}
			}
			None => Err(InsertionError {
				item,
				child_offset: 0,
				child_id: self.first_child,
			}),
		}
	}

	// /// Get the offset of the item with the given key.
	// #[inline]
	// pub fn key_offset(&self, key: &K) -> Result<usize, (usize, usize)> {
	// 	match binary_search_min(&self.other_children, key) {
	// 		Some(i) => {
	// 			if self.other_children[i].item.key() == key {
	// 				Ok(i)
	// 			} else {
	// 				Err((i+1, self.other_children[i].child))
	// 			}
	// 		},
	// 		None => {
	// 			Err((0, self.first_child))
	// 		}
	// 	}
	// }

	/// Insert item at the given offset.
	#[inline]
	pub fn insert(&mut self, offset: Offset, item: T, right_node_id: S::Node) {
		self.other_children.insert(
			offset.unwrap(),
			Branch {
				item,
				child: right_node_id,
			},
		);
	}

	/// Replace the item at the given offset.
	#[inline]
	pub fn replace(&mut self, offset: Offset, mut item: T) -> T {
		std::mem::swap(&mut item, &mut self.other_children[offset.unwrap()].item);
		item
	}

	/// Remove the item at the given offset.
	/// Return the child id on the left of the item, the item, and the child id on the right
	/// (which is also removed).
	#[inline]
	pub fn remove(&mut self, offset: Offset) -> (S::Node, T, S::Node) {
		let offset = offset.unwrap();
		let b = self.other_children.remove(offset).unwrap();
		let left_child_id = self.child_id(offset);
		(left_child_id, b.item, b.child)
	}

	#[inline]
	pub fn split(&mut self) -> (usize, T, Internal<T, S>) {
		assert!(self.is_overflowing()); // implies self.other_children.len() >= 4

		// Index of the median-key item in `other_children`.
		let median_i = (self.other_children.len() - 1) / 2; // Since M is at least 3, `median_i` is at least 1.

		let right_other_children = self.other_children.drain(median_i + 1..).collect();
		let median = self.other_children.pop().unwrap();

		let right_node = Internal {
			parent: self.parent,
			first_child: median.child,
			other_children: right_other_children,
		};

		assert!(!self.is_underflowing());
		assert!(!right_node.is_underflowing());

		(self.other_children.len(), median.item, right_node)
	}

	/// Merge the children at the given indexes.
	///
	/// It is supposed that `left_index` is `right_index-1`.
	/// This method returns the identifier of the left node in the tree, the identifier of the right node,
	/// the item removed from this node to be merged with the merged children and
	/// the balance status of this node after the merging operation.
	#[inline]
	pub fn merge(&mut self, left_index: usize) -> (usize, S::Node, S::Node, T, Balance) {
		// We remove the right child (the one of index `right_index`).
		// Since left_index = right_index-1, it is indexed by `left_index` in `other_children`.
		let branch = self.other_children.remove(left_index).unwrap();
		let left_id = self.child_id(left_index);
		let right_id = branch.child;
		(left_index, left_id, right_id, branch.item, self.balance())
	}

	#[inline]
	pub fn push_left(&mut self, item: T, mut child_id: S::Node) {
		std::mem::swap(&mut self.first_child, &mut child_id);
		self.other_children.insert(
			0,
			Branch {
				item,
				child: child_id,
			},
		);
	}

	#[inline]
	pub fn pop_left(&mut self) -> Result<(T, S::Node), WouldUnderflow> {
		if self.item_count() <= UNDERFLOW {
			Err(WouldUnderflow)
		} else {
			let first = self.other_children.remove(0).unwrap();
			let child_id = std::mem::replace(&mut self.first_child, first.child);
			Ok((first.item, child_id))
		}
	}

	#[inline]
	pub fn push_right(&mut self, item: T, child_id: S::Node) -> Offset {
		let offset = self.other_children.len();
		self.other_children.push(Branch {
			item,
			child: child_id,
		});
		offset.into()
	}

	#[inline]
	pub fn pop_right(&mut self) -> Result<RightBranch<T, S>, WouldUnderflow> {
		if self.item_count() <= UNDERFLOW {
			Err(WouldUnderflow)
		} else {
			let offset = self.other_children.len();
			let last = self.other_children.pop().unwrap();
			Ok(RightBranch {
				offset: offset.into(),
				item: last.item,
				child: last.child,
			})
		}
	}

	#[inline]
	pub fn append(&mut self, separator: T, mut other: Internal<T, S>) -> Offset {
		let offset = self.other_children.len();
		self.other_children.push(Branch {
			item: separator,
			child: other.first_child,
		});

		self.other_children.append(&mut other.other_children);
		offset.into()
	}

	/// Write the label of the internal node in the DOT format.
	///
	/// Requires the `dot` feature.
	#[cfg(feature = "dot")]
	#[inline]
	pub fn dot_write_label<W: std::io::Write>(&self, f: &mut W) -> std::io::Result<()>
	where
		T: std::fmt::Display,
	{
		write!(f, "<c0> |")?;
		let mut i = 1;
		for branch in &self.other_children {
			write!(f, "{{{}|<c{}>}} |", branch.item, i)?;
			i += 1;
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
				Balance::Overflow => panic!("internal node is overflowing"),
				Balance::Underflow(_) => panic!("internal node is underflowing"),
				_ => (),
			}
		} else if self.item_count() == 0 {
			panic!("root node is empty")
		}

		if !self
			.other_children
			.windows(2)
			.all(|w| cmp(&w[0].item, &w[1].item).is_lt())
		{
			panic!("internal node items are not sorted")
		}

		if let Some(min) = min {
			if let Some(b) = self.other_children.first() {
				if cmp(min, &b.item).is_ge() {
					panic!("internal node item key is greater than right separator")
				}
			}
		}

		if let Some(max) = max {
			if let Some(b) = self.other_children.last() {
				if cmp(max, &b.item).is_le() {
					panic!("internal node item key is less than left separator")
				}
			}
		}
	}
}

pub struct RightBranch<T, S: Storage<T>> {
	pub offset: Offset,
	pub item: T,
	pub child: S::Node,
}
