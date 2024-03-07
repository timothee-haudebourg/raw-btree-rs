use crate::{
	balancing::rebalance,
	node::{Address, Offset},
	utils::Array,
	Node, M,
};
use core::fmt;
use std::cmp::Ordering;

pub trait Storage<T>: Default {
	/// Node.
	type Node: Copy + PartialEq + core::fmt::Debug;

	/// Nodes dropper.
	type Dropper: Dropper<T, Self>;

	/// Allocates the given node.
	fn allocate_node(&mut self, node: Node<T, Self>) -> Self::Node;

	/// Inserts the given node into the storage, setting the children parent.
	///
	/// # Safety
	///
	/// The input node's children must not have been deallocated.
	unsafe fn insert_node(&mut self, node: Node<T, Self>) -> Self::Node {
		let children: Array<Self::Node, M> = node.children().collect();
		let id = self.allocate_node(node);
		for child_id in children {
			self.get_mut(child_id).set_parent(Some(id));
		}

		id
	}

	/// # Safety
	///
	/// Input node must not have been deallocated.
	unsafe fn release_node(&mut self, id: Self::Node) -> Node<T, Self>;

	/// Creates a new dropper.
	///
	/// Returns `None` if no dropper is required to eventually drop all the
	/// nodes.
	fn start_dropping(&self) -> Option<Self::Dropper>;

	/// # Safety
	///
	/// Input node must not have been deallocated.
	unsafe fn get(&self, id: Self::Node) -> &Node<T, Self>;

	/// # Safety
	///
	/// - Input node must not have been deallocated.
	/// - Different `id` must map to non-aliased nodes.
	/// - Must not be used to create more than one concurrent mutable reference
	///   to the same node.
	unsafe fn get_mut(&mut self, id: Self::Node) -> &mut Node<T, Self>;

	/// Normalizes the given address.
	///
	/// # Safety
	///
	/// Input address's node must not have been deallocated.
	unsafe fn normalize(&self, mut addr: Address<Self::Node>) -> Option<Address<Self::Node>> {
		loop {
			let node = self.get(addr.node);
			if addr.offset >= node.item_count() {
				match node.parent() {
					Some(parent_id) => {
						addr.offset = self.get(parent_id).child_index(addr.node).unwrap().into();
						addr.node = parent_id;
					}
					None => break None,
				}
			} else {
				break Some(addr);
			}
		}
	}

	/// Converts this arbitrary address into a leaf address.
	///
	/// # Safety
	///
	/// Input address's node must not have been deallocated.
	#[inline]
	unsafe fn leaf_address(&self, mut addr: Address<Self::Node>) -> Address<Self::Node> {
		loop {
			let node = self.get(addr.node);
			match node.child_id_opt(addr.offset.unwrap()) {
				// TODO unwrap may fail here!
				Some(child_id) => {
					addr.node = child_id;
					addr.offset = self.get(child_id).item_count().into()
				}
				None => break,
			}
		}

		addr
	}

	/// Get the address of the item located before this address.
	///
	/// # Safety
	///
	/// Input address's node must not have been deallocated.
	#[inline]
	unsafe fn previous_item_address(
		&self,
		mut addr: Address<Self::Node>,
	) -> Option<Address<Self::Node>> {
		loop {
			let node = self.get(addr.node);

			match node.child_id_opt(addr.offset.unwrap()) {
				// TODO unwrap may fail here.
				Some(child_id) => {
					addr.offset = self.get(child_id).item_count().into();
					addr.node = child_id;
				}
				None => loop {
					if addr.offset > 0 {
						addr.offset.decr();
						return Some(addr);
					}

					match self.get(addr.node).parent() {
						Some(parent_id) => {
							addr.offset =
								self.get(parent_id).child_index(addr.node).unwrap().into();
							addr.node = parent_id;
						}
						None => return None,
					}
				},
			}
		}
	}

	/// Returns the front address directly preceding the given address.
	///
	/// # Safety
	///
	/// Input address's node must not have been deallocated.
	#[inline]
	unsafe fn previous_front_address(
		&self,
		mut addr: Address<Self::Node>,
	) -> Option<Address<Self::Node>> {
		loop {
			let node = self.get(addr.node);
			match addr.offset.value() {
				Some(offset) => {
					let index = if offset < node.item_count() {
						offset
					} else {
						node.item_count()
					};

					match node.child_id_opt(index) {
						Some(child_id) => {
							addr.offset = (self.get(child_id).item_count()).into();
							addr.node = child_id;
						}
						None => {
							addr.offset.decr();
							break;
						}
					}
				}
				None => match node.parent() {
					Some(parent_id) => {
						addr.offset = self.get(parent_id).child_index(addr.node).unwrap().into();
						addr.offset.decr();
						addr.node = parent_id;
						break;
					}
					None => return None,
				},
			}
		}

		Some(addr)
	}

	/// Get the address of the item located after this address if any.
	///
	/// # Safety
	///
	/// Input address's node must not have been deallocated.
	#[inline]
	unsafe fn next_item_address(
		&self,
		mut addr: Address<Self::Node>,
	) -> Option<Address<Self::Node>> {
		let item_count = self.get(addr.node).item_count();
		match addr.offset.partial_cmp(&item_count) {
			Some(std::cmp::Ordering::Less) => {
				addr.offset.incr();
			}
			Some(std::cmp::Ordering::Greater) => {
				return None;
			}
			_ => (),
		}

		// let original_addr_shifted = addr;

		loop {
			let node = self.get(addr.node);

			match node.child_id_opt(addr.offset.unwrap()) {
				// unwrap may fail here.
				Some(child_id) => {
					addr.offset = 0.into();
					addr.node = child_id;
				}
				None => {
					loop {
						let node = self.get(addr.node);

						if addr.offset < node.item_count() {
							return Some(addr);
						}

						match node.parent() {
							Some(parent_id) => {
								addr.offset =
									self.get(parent_id).child_index(addr.node).unwrap().into();
								addr.node = parent_id;
							}
							None => {
								// return Some(original_addr_shifted)
								return None;
							}
						}
					}
				}
			}
		}
	}

	//// Returns the back address directly following the given address.
	///
	/// # Safety
	///
	/// Input address's node must not have been deallocated.
	#[inline]
	unsafe fn next_back_address(
		&self,
		mut addr: Address<Self::Node>,
	) -> Option<Address<Self::Node>> {
		loop {
			let node = self.get(addr.node);
			let index = match addr.offset.value() {
				Some(offset) => offset + 1,
				None => 0,
			};

			if index <= node.item_count() {
				match node.child_id_opt(index) {
					Some(child_id) => {
						addr.offset = Offset::before();
						addr.node = child_id;
					}
					None => {
						addr.offset = index.into();
						break;
					}
				}
			} else {
				match node.parent() {
					Some(parent_id) => {
						addr.offset = self.get(parent_id).child_index(addr.node).unwrap().into();
						addr.node = parent_id;
						break;
					}
					None => return None,
				}
			}
		}

		Some(addr)
	}

	/// Returns the item address or back address directly following the given
	/// address.
	///
	/// # Safety
	///
	/// Input address's node must not have been deallocated.
	#[inline]
	unsafe fn next_item_or_back_address(
		&self,
		mut addr: Address<Self::Node>,
	) -> Option<Address<Self::Node>> {
		let item_count = self.get(addr.node).item_count();
		match addr.offset.partial_cmp(&item_count) {
			Some(std::cmp::Ordering::Less) => {
				addr.offset.incr();
			}
			Some(std::cmp::Ordering::Greater) => {
				return None;
			}
			_ => (),
		}

		let original_addr_shifted = addr;

		loop {
			let node = self.get(addr.node);

			match node.child_id_opt(addr.offset.unwrap()) {
				// TODO unwrap may fail here.
				Some(child_id) => {
					addr.offset = 0.into();
					addr.node = child_id;
				}
				None => loop {
					let node = self.get(addr.node);

					if addr.offset < node.item_count() {
						return Some(addr);
					}

					match node.parent() {
						Some(parent_id) => {
							addr.offset =
								self.get(parent_id).child_index(addr.node).unwrap().into();
							addr.node = parent_id;
						}
						None => return Some(original_addr_shifted),
					}
				},
			}
		}
	}

	/// # Safety
	///
	/// Input node must not have been deallocated.
	unsafe fn address_in<Q: ?Sized>(
		&self,
		mut id: Self::Node,
		cmp: impl Fn(&T, &Q) -> Ordering,
		key: &Q,
	) -> Result<Address<Self::Node>, Address<Self::Node>> {
		loop {
			match self.get(id).offset_of(&cmp, key) {
				Ok(offset) => return Ok(Address { node: id, offset }),
				Err((offset, None)) => return Err(Address::new(id, offset.into())),
				Err((_, Some(child_id))) => {
					id = child_id;
				}
			}
		}
	}

	/// Inserts the item at the given address.
	///
	/// # Safety
	///
	/// Input nodes must not have been deallocated.
	unsafe fn insert_at(
		&mut self,
		root: Option<Self::Node>,
		addr: Option<Address<Self::Node>>,
		item: T,
	) -> (Option<Self::Node>, Option<Address<Self::Node>>) {
		self.insert_exactly_at(root, addr.map(|addr| self.leaf_address(addr)), item, None)
	}

	/// Inserts the given item exactly at the provided **leaf** address.
	///
	/// # Safety
	///
	/// Input nodes must not have been deallocated.
	unsafe fn insert_exactly_at(
		&mut self,
		root: Option<Self::Node>,
		addr: Option<Address<Self::Node>>,
		item: T,
		opt_right_id: Option<Self::Node>,
	) -> (Option<Self::Node>, Option<Address<Self::Node>>) {
		match addr {
			Some(addr) => {
				self.get_mut(addr.node)
					.insert(addr.offset, item, opt_right_id);
				rebalance(self, root, addr.node, addr)
			}
			None => {
				let new_root = Node::leaf(None, item);
				let id = self.insert_node(new_root);
				let addr = Address {
					node: id,
					offset: 0.into(),
				};
				(Some(id), Some(addr))
			}
		}
	}

	/// Replaces the item located at the given address.
	///
	/// # Safety
	///
	/// Input address's node must not have been deallocated.
	unsafe fn replace_at(&mut self, addr: Address<Self::Node>, item: T) -> T {
		std::mem::replace(self.get_mut(addr.node).item_mut(addr.offset).unwrap(), item)
	}

	/// # Safety
	///
	/// Input nodes must not have been deallocated.
	#[inline]
	unsafe fn remove_at(
		&mut self,
		root: Option<Self::Node>,
		addr: Address<Self::Node>,
	) -> Option<RemovedItem<T, Self>> {
		match self.get_mut(addr.node).leaf_remove(addr.offset) {
			Some(Ok(item)) => {
				// removed from a leaf.
				let (new_root, new_addr) = rebalance(self, root, addr.node, addr);
				Some(RemovedItem {
					new_root,
					item,
					new_addr,
				})
			}
			Some(Err(left_child_id)) => {
				// removed from an internal node.
				let new_addr = self.next_item_or_back_address(addr).unwrap();
				let (separator, leaf_id) = self.remove_rightmost_leaf_of(left_child_id);
				let item = self.get_mut(addr.node).replace(addr.offset, separator);
				let (new_root, new_addr) = rebalance(self, root, leaf_id, new_addr);
				Some(RemovedItem {
					new_root,
					item,
					new_addr,
				})
			}
			None => None,
		}
	}

	/// Remove the rightmost leaf node under the given node.
	///
	/// # Safety
	///
	/// Input node must not have been deallocated.
	#[inline]
	unsafe fn remove_rightmost_leaf_of(&mut self, mut id: Self::Node) -> (T, Self::Node) {
		loop {
			match self.get_mut(id).remove_rightmost_leaf() {
				Ok(result) => return (result, id),
				Err(child_id) => {
					id = child_id;
				}
			}
		}
	}
}

pub struct RemovedItem<T, S: Storage<T>> {
	pub new_root: Option<S::Node>,
	pub item: T,
	pub new_addr: Option<Address<S::Node>>,
}

/// Storage dropper.
///
/// Used to drop all the nodes of a node storage.
pub trait Dropper<T, S: Storage<T>>: Sized {
	/// Drops the given node.
	///
	/// # Safety
	///
	/// - The node must not have been deallocated.
	/// - No reference to the node or the node's content must exist.
	/// - The node cannot be dereferenced anymore.
	unsafe fn drop_node(&mut self, id: S::Node);
}

#[derive(Default)]
pub struct BoxStorage;

pub struct BoxPtr<T, S: Storage<T>>(*mut Node<T, S>);

impl<T> Storage<T> for BoxStorage {
	type Node = BoxPtr<T, Self>;

	type Dropper = BoxDrop;

	fn allocate_node(&mut self, node: Node<T, Self>) -> Self::Node {
		let b = Box::new(node);
		BoxPtr(Box::into_raw(b))
	}

	unsafe fn release_node(&mut self, id: Self::Node) -> Node<T, Self> {
		let b = Box::from_raw(id.0);
		*b
	}

	fn start_dropping(&self) -> Option<Self::Dropper> {
		Some(BoxDrop)
	}

	unsafe fn get(&self, id: Self::Node) -> &Node<T, Self> {
		&*id.0
	}

	unsafe fn get_mut(&mut self, id: Self::Node) -> &mut Node<T, Self> {
		&mut *id.0
	}
}

impl<T, S: Storage<T>> fmt::Debug for BoxPtr<T, S> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}

impl<T, S: Storage<T>> Clone for BoxPtr<T, S> {
	fn clone(&self) -> Self {
		*self
	}
}

impl<T, S: Storage<T>> Copy for BoxPtr<T, S> {}

impl<T, S: Storage<T>> PartialEq for BoxPtr<T, S> {
	fn eq(&self, other: &Self) -> bool {
		self.0 == other.0
	}
}

impl<T, S: Storage<T>> Eq for BoxPtr<T, S> {}

impl<T, S: Storage<T>> From<BoxPtr<T, S>> for usize {
	fn from(value: BoxPtr<T, S>) -> Self {
		value.0 as usize
	}
}

pub struct BoxDrop;

impl<T> Dropper<T, BoxStorage> for BoxDrop {
	unsafe fn drop_node(&mut self, id: BoxPtr<T, BoxStorage>) {
		let _ = Box::from_raw(id.0);
	}
}
