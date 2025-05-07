//! This library provides a [`RawBTree`] type that can be used as a basis for
//! any B-Tree-based data structure.
//!
//! [`RawBTree`]: crate::RawBTree
pub(crate) mod utils;

pub mod node;
pub use node::{Address, Node};
use std::{cmp::Ordering, iter::FusedIterator, marker::PhantomData};

mod balancing;
mod item;
pub mod storage;

pub use item::Item;
use storage::BoxStorage;
pub use storage::Storage;

use crate::utils::Array;

/// Knuth order of the B-Trees.
///
/// Must be at least 4.
pub const M: usize = 8;

pub struct RawBTree<T, S: Storage<T> = BoxStorage> {
	/// Allocated and free nodes.
	nodes: S,

	/// Root node.
	root: Option<S::Node>,

	/// Number of items in the tree.
	len: usize,

	item: PhantomData<T>,
}

impl<T, S: Storage<T>> Default for RawBTree<T, S> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T, S: Storage<T>> RawBTree<T, S> {
	/// Create a new empty B-tree.
	#[inline]
	pub fn new() -> RawBTree<T, S> {
		RawBTree {
			nodes: Default::default(),
			root: None,
			len: 0,
			item: PhantomData,
		}
	}

	#[inline]
	pub fn is_empty(&self) -> bool {
		self.root.is_none()
	}

	#[inline]
	pub fn len(&self) -> usize {
		self.len
	}

	pub fn address_of<Q: ?Sized>(
		&self,
		cmp: impl Fn(&T, &Q) -> Ordering,
		key: &Q,
	) -> Result<Address<S::Node>, Option<Address<S::Node>>> {
		match self.root {
			Some(id) => unsafe { self.nodes.address_in(id, cmp, key).map_err(Some) },
			None => Err(None),
		}
	}

	pub fn first_item_address(&self) -> Option<Address<S::Node>> {
		self.root.map(|mut id| unsafe {
			loop {
				match self.nodes.get(id).child_id_opt(0) {
					Some(child_id) => id = child_id,
					None => break Address::new(id, 0.into()),
				}
			}
		})
	}

	// fn first_back_address(&self) -> Address {
	// 	match self.root {
	// 		Some(mut id) => loop {
	// 			match self.node(id).child_id_opt(0) {
	// 				Some(child_id) => id = child_id,
	// 				None => return Address::new(id, 0.into()), // TODO FIXME thechnically not the first
	// 			}
	// 		},
	// 		None => Address::nowhere(),
	// 	}
	// }

	fn last_item_address(&self) -> Option<Address<S::Node>> {
		self.root.map(|mut id| unsafe {
			loop {
				let node = self.nodes.get(id);
				let index = node.item_count();
				match node.child_id_opt(index) {
					Some(child_id) => id = child_id,
					None => break Address::new(id, (index - 1).into()),
				}
			}
		})
	}

	// fn last_valid_address(&self) -> Address {
	// 	match self.root {
	// 		Some(mut id) => loop {
	// 			let node = self.node(id);
	// 			let index = node.item_count();
	// 			match node.child_id_opt(index) {
	// 				Some(child_id) => id = child_id,
	// 				None => return Address::new(id, index.into()),
	// 			}
	// 		},
	// 		None => Address::nowhere(),
	// 	}
	// }

	/// Return the item at the given address.
	///
	/// # Safety
	///
	/// The address's node must not have been deallocated.
	#[inline]
	pub unsafe fn get_at(&self, addr: Address<S::Node>) -> Option<&T> {
		self.nodes.get(addr.node).item(addr.offset)
	}

	/// Returns a mutable reference to the item at the given address.
	///
	/// # Safety
	///
	/// The address's node must not have been deallocated.
	#[inline]
	pub unsafe fn get_mut_at(&mut self, addr: Address<S::Node>) -> Option<&mut T> {
		self.nodes.get_mut(addr.node).item_mut(addr.offset)
	}

	#[inline]
	pub fn get<Q: ?Sized>(&self, cmp: impl Fn(&T, &Q) -> Ordering, key: &Q) -> Option<&T> {
		self.address_of(cmp, key)
			.ok()
			.and_then(|addr| unsafe { self.get_at(addr) })
	}

	#[inline]
	pub fn get_mut<Q: ?Sized>(
		&mut self,
		cmp: impl Fn(&T, &Q) -> Ordering,
		key: &Q,
	) -> Option<&mut T> {
		self.address_of(cmp, key)
			.ok()
			.and_then(|addr| unsafe { self.get_mut_at(addr) })
	}

	#[inline]
	pub fn first(&self) -> Option<&T> {
		self.first_item_address()
			.and_then(|addr| unsafe { self.get_at(addr) })
	}

	#[inline]
	pub fn first_mut(&mut self) -> Option<&mut T> {
		self.first_item_address()
			.and_then(|addr| unsafe { self.get_mut_at(addr) })
	}

	#[inline]
	pub fn last(&self) -> Option<&T> {
		self.last_item_address()
			.and_then(|addr| unsafe { self.get_at(addr) })
	}

	#[inline]
	pub fn last_mut(&mut self) -> Option<&mut T> {
		self.last_item_address()
			.and_then(|addr| unsafe { self.get_mut_at(addr) })
	}

	pub fn iter(&self) -> Iter<T, S> {
		Iter::new(self)
	}

	pub fn iter_mut(&mut self) -> IterMut<T, S> {
		IterMut::new(self)
	}

	#[inline]
	pub fn insert(&mut self, cmp: impl Fn(&T, &T) -> Ordering, item: T) -> Option<T> {
		match self.address_of(cmp, &item) {
			Ok(addr) => Some(unsafe { self.nodes.replace_at(addr, item) }),
			Err(addr) => {
				let (root, _) =
					unsafe { self.nodes.insert_exactly_at(self.root, addr, item, None) };
				self.root = root;
				self.len += 1;
				None
			}
		}
	}

	/// Remove the next item and return it.
	#[inline]
	pub fn remove<Q: ?Sized>(&mut self, cmp: impl Fn(&T, &Q) -> Ordering, key: &Q) -> Option<T> {
		match self.address_of(cmp, key) {
			Ok(addr) => {
				let r = unsafe { self.nodes.remove_at(self.root, addr).unwrap() };
				self.root = r.new_root;
				self.len -= 1;
				Some(r.item)
			}
			Err(_) => None,
		}
	}

	/// Removes the item at the given address and returns it.
	///
	/// # Safety
	///
	/// Target node must not have been deallocated.
	#[inline]
	pub unsafe fn remove_at<Q: ?Sized>(&mut self, addr: Address<S::Node>) -> T {
		let r = unsafe { self.nodes.remove_at(self.root, addr).unwrap() };
		self.root = r.new_root;
		self.len -= 1;
		r.item
	}

	pub fn visit_from_leaves(&self, mut f: impl FnMut(S::Node)) {
		if let Some(id) = self.root {
			let node = unsafe { self.nodes.get(id) };
			node.visit_from_leaves(&self.nodes, &mut f);
			f(id)
		}
	}

	pub fn visit_from_leaves_mut(&mut self, mut f: impl FnMut(S::Node, &mut Node<T, S>)) {
		if let Some(root_id) = self.root {
			let root_node: &mut Node<T, S> =
				unsafe { std::mem::transmute(self.nodes.get_mut(root_id)) };
			root_node.visit_from_leaves_mut(&mut self.nodes, &mut f);
			f(root_id, root_node)
		}
	}

	pub fn forget(&mut self) {
		use storage::Dropper;
		let mut dropper = self.nodes.start_dropping();

		self.visit_from_leaves_mut(|id, node| unsafe {
			node.forget();
			if let Some(dropper) = &mut dropper {
				dropper.drop_node(id);
			}
		});

		self.root = None;
		self.len = 0;
		self.nodes = S::default();
	}

	pub fn clear(&mut self) {
		use storage::Dropper;
		if let Some(mut dropper) = self.nodes.start_dropping() {
			self.visit_from_leaves(|id| unsafe { dropper.drop_node(id) })
		}

		self.root = None;
		self.len = 0;
		self.nodes = S::default();
	}

	#[cfg(debug_assertions)]
	pub fn validate(&self, cmp: impl Fn(&T, &T) -> Ordering) {
		if let Some(id) = self.root {
			self.validate_node(&cmp, id, None, None, None);
		}
	}

	/// Validate the given node and returns the depth of the node.
	#[cfg(debug_assertions)]
	pub fn validate_node(
		&self,
		cmp: &impl Fn(&T, &T) -> Ordering,
		id: S::Node,
		parent: Option<S::Node>,
		mut min: Option<&T>,
		mut max: Option<&T>,
	) -> usize {
		let node = unsafe { self.nodes.get(id) };
		node.validate(cmp, parent, min, max);

		let mut depth = None;
		for (i, child_id) in node.children().enumerate() {
			let (child_min, child_max) = node.separators(i);
			let min = child_min.or_else(|| min.take());
			let max = child_max.or_else(|| max.take());

			let child_depth = self.validate_node(cmp, child_id, Some(id), min, max);
			match depth {
				None => depth = Some(child_depth),
				Some(depth) => {
					if depth != child_depth {
						panic!("tree not balanced")
					}
				}
			}
		}

		match depth {
			Some(depth) => depth + 1,
			None => 0,
		}
	}

	/// Write the tree in the DOT graph descrption language.
	///
	/// Requires the `dot` feature.
	#[cfg(feature = "dot")]
	#[inline]
	pub fn dot_write<W: std::io::Write>(&self, f: &mut W) -> std::io::Result<()>
	where
		T: std::fmt::Display,
		S::Node: Into<usize>,
	{
		write!(f, "digraph tree {{\n\tnode [shape=record];\n")?;
		if let Some(id) = self.root {
			self.dot_write_node(f, id)?
		}
		write!(f, "}}")
	}

	/// Write the given node in the DOT graph descrption language.
	///
	/// Requires the `dot` feature.
	#[cfg(feature = "dot")]
	#[inline]
	fn dot_write_node<W: std::io::Write>(&self, f: &mut W, id: S::Node) -> std::io::Result<()>
	where
		T: std::fmt::Display,
		S::Node: Into<usize>,
	{
		let name = format!("n{:?}", id.into());
		let node = unsafe { self.nodes.get(id) };

		write!(f, "\t{} [label=\"", name)?;
		if let Some(parent) = node.parent() {
			write!(f, "({:?})|", parent.into())?;
		}

		node.dot_write_label(f)?;
		writeln!(f, "({:?})\"];", id.into())?;

		for child_id in node.children() {
			self.dot_write_node(f, child_id)?;
			let child_name = format!("n{:?}", child_id.into());
			writeln!(f, "\t{} -> {}", name, child_name)?;
		}

		Ok(())
	}
}

impl<T, S: Storage<T>> Drop for RawBTree<T, S> {
	fn drop(&mut self) {
		self.clear();
	}
}

impl<T: Clone, S: Storage<T>> Clone for RawBTree<T, S> {
	fn clone(&self) -> Self {
		unsafe fn clone_node<T: Clone, S: Storage<T>>(
			old_nodes: &S,
			new_nodes: &mut S,
			parent: Option<S::Node>,
			node_id: S::Node,
		) -> S::Node {
			let clone = match old_nodes.get(node_id) {
				Node::Leaf(node) => Node::Leaf(node::LeafNode::new(parent, node.items().clone())),
				Node::Internal(node) => {
					let first = clone_node(old_nodes, new_nodes, None, node.first_child_id());
					let mut branches = Array::new();
					for b in node.branches() {
						branches.push(node::internal::Branch {
							item: b.item.clone(),
							child: clone_node(old_nodes, new_nodes, None, b.child),
						})
					}

					Node::Internal(node::InternalNode::new(parent, first, branches))
				}
			};

			new_nodes.insert_node(clone)
		}

		let mut nodes = S::default();
		let root = self
			.root
			.map(|root| unsafe { clone_node(&self.nodes, &mut nodes, None, root) });

		Self {
			nodes,
			root,
			len: self.len,
			item: PhantomData,
		}
	}
}

pub struct Iter<'a, T, S: Storage<T> = BoxStorage> {
	/// The tree reference.
	btree: &'a RawBTree<T, S>,

	/// Address of the next item.
	addr: Option<Address<S::Node>>,

	/// End address.
	end: Option<Address<S::Node>>,

	/// Remaining item count.
	len: usize,
}

impl<'a, T, S: Storage<T>> Iter<'a, T, S> {
	#[inline]
	fn new(btree: &'a RawBTree<T, S>) -> Self {
		let addr = btree.first_item_address();
		let len = btree.len();
		Iter {
			btree,
			addr,
			end: None,
			len,
		}
	}
}

impl<'a, T, S: Storage<T>> Iterator for Iter<'a, T, S> {
	type Item = &'a T;

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		(self.len, Some(self.len))
	}

	#[inline]
	fn next(&mut self) -> Option<&'a T> {
		match self.addr {
			Some(addr) => unsafe {
				if self.len > 0 {
					self.len -= 1;

					let item = self.btree.get_at(addr).unwrap();
					self.addr = self.btree.nodes.next_item_address(addr);
					Some(item)
				} else {
					None
				}
			},
			None => None,
		}
	}
}

impl<'a, T, S: Storage<T>> FusedIterator for Iter<'a, T, S> {}
impl<'a, T, S: Storage<T>> ExactSizeIterator for Iter<'a, T, S> {}

impl<'a, T, S: Storage<T>> DoubleEndedIterator for Iter<'a, T, S> {
	#[inline]
	fn next_back(&mut self) -> Option<&'a T> {
		if self.len > 0 {
			unsafe {
				let addr = match self.end {
					Some(addr) => self.btree.nodes.previous_item_address(addr).unwrap(),
					None => self.btree.last_item_address().unwrap(),
				};

				self.len -= 1;

				let item = self.btree.get_at(addr).unwrap();
				self.end = Some(addr);
				Some(item)
			}
		} else {
			None
		}
	}
}

impl<'a, T, S: Storage<T>> Clone for Iter<'a, T, S> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<'a, T, S: Storage<T>> Copy for Iter<'a, T, S> {}

impl<'a, T, S: Storage<T>> IntoIterator for &'a RawBTree<T, S> {
	type IntoIter = Iter<'a, T, S>;
	type Item = &'a T;

	#[inline]
	fn into_iter(self) -> Iter<'a, T, S> {
		self.iter()
	}
}

pub struct IterMut<'a, T, S: Storage<T> = BoxStorage> {
	/// The tree reference.
	btree: &'a mut RawBTree<T, S>,

	/// Address of the next item.
	addr: Option<Address<S::Node>>,

	/// End address.
	end: Option<Address<S::Node>>,

	/// Remaining item count.
	len: usize,
}

impl<'a, T, S: Storage<T>> IterMut<'a, T, S> {
	#[inline]
	fn new(btree: &'a mut RawBTree<T, S>) -> Self {
		let addr = btree.first_item_address();
		let len = btree.len();
		Self {
			btree,
			addr,
			end: None,
			len,
		}
	}
}

impl<'a, T, S: Storage<T>> Iterator for IterMut<'a, T, S> {
	type Item = &'a mut T;

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		(self.len, Some(self.len))
	}

	#[inline]
	fn next(&mut self) -> Option<&'a mut T> {
		match self.addr {
			Some(addr) => unsafe {
				if self.len > 0 {
					self.len -= 1;
					self.addr = self.btree.nodes.next_item_address(addr);
					Some(std::mem::transmute::<&mut T, &'a mut T>(
						self.btree.get_mut_at(addr).unwrap(),
					))
				} else {
					None
				}
			},
			None => None,
		}
	}
}

impl<'a, T, S: Storage<T>> FusedIterator for IterMut<'a, T, S> {}
impl<'a, T, S: Storage<T>> ExactSizeIterator for IterMut<'a, T, S> {}

impl<'a, T, S: Storage<T>> DoubleEndedIterator for IterMut<'a, T, S> {
	#[inline]
	fn next_back(&mut self) -> Option<&'a mut T> {
		if self.len > 0 {
			unsafe {
				let addr = match self.end {
					Some(addr) => self.btree.nodes.previous_item_address(addr).unwrap(),
					None => self.btree.last_item_address().unwrap(),
				};

				self.len -= 1;
				self.end = Some(addr);
				Some(std::mem::transmute::<&mut T, &'a mut T>(
					self.btree.get_mut_at(addr).unwrap(),
				))
			}
		} else {
			None
		}
	}
}

impl<'a, T, S: Storage<T>> IntoIterator for &'a mut RawBTree<T, S> {
	type IntoIter = IterMut<'a, T, S>;
	type Item = &'a mut T;

	#[inline]
	fn into_iter(self) -> IterMut<'a, T, S> {
		self.iter_mut()
	}
}

pub struct IntoIter<T, S: Storage<T> = BoxStorage> {
	/// The tree.
	btree: RawBTree<T, S>,

	/// Address of the next item.
	addr: Option<Address<S::Node>>,

	/// End address.
	end: Option<Address<S::Node>>,

	/// Remaining item count.
	len: usize,
}

impl<T, S: Storage<T>> IntoIter<T, S> {
	#[inline]
	fn new(btree: RawBTree<T, S>) -> Self {
		let addr = btree.first_item_address();
		let len = btree.len();
		Self {
			btree,
			addr,
			end: None,
			len,
		}
	}
}

impl<T, S: Storage<T>> Iterator for IntoIter<T, S> {
	type Item = T;

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		(self.len, Some(self.len))
	}

	#[inline]
	fn next(&mut self) -> Option<T> {
		match self.addr {
			Some(addr) => unsafe {
				if self.len > 0 {
					self.len -= 1;
					self.addr = self.btree.nodes.next_item_address(addr);
					Some(std::ptr::read(self.btree.get_at(addr).unwrap()))
				} else {
					None
				}
			},
			None => None,
		}
	}
}

impl<T, S: Storage<T>> FusedIterator for IntoIter<T, S> {}
impl<T, S: Storage<T>> ExactSizeIterator for IntoIter<T, S> {}

impl<T, S: Storage<T>> DoubleEndedIterator for IntoIter<T, S> {
	#[inline]
	fn next_back(&mut self) -> Option<T> {
		if self.len > 0 {
			unsafe {
				let addr = match self.end {
					Some(addr) => self.btree.nodes.previous_item_address(addr).unwrap(),
					None => self.btree.last_item_address().unwrap(),
				};

				self.len -= 1;
				self.end = Some(addr);
				Some(std::ptr::read(self.btree.get_at(addr).unwrap()))
			}
		} else {
			None
		}
	}
}

impl<T, S: Storage<T>> IntoIterator for RawBTree<T, S> {
	type IntoIter = IntoIter<T, S>;
	type Item = T;

	#[inline]
	fn into_iter(self) -> IntoIter<T, S> {
		IntoIter::new(self)
	}
}

impl<T, S: Storage<T>> Drop for IntoIter<T, S> {
	fn drop(&mut self) {
		let _ = self.last();
		self.btree.forget();
	}
}
