use std::cmp::Ordering;

use crate::{
	node::{Address, Balance, WouldUnderflow},
	Node, Storage,
};

/// # Safety
///
/// Input nodes must not have been deallocated.
#[inline]
pub unsafe fn rebalance<T, S: Storage<T>>(
	tree: &mut S,
	mut root: Option<S::Node>,
	mut id: S::Node,
	mut addr: Address<S::Node>,
) -> (Option<S::Node>, Option<Address<S::Node>>) {
	let mut balance = tree.get(id).balance();

	let addr = loop {
		match balance {
			Balance::Balanced => break Some(addr),
			Balance::Overflow => {
				assert!(!tree.get_mut(id).is_underflowing());
				let (median_offset, median, right_node) = tree.get_mut(id).split();
				let right_id = tree.insert_node(right_node);

				match tree.get(id).parent() {
					Some(parent_id) => {
						let parent = tree.get_mut(parent_id);
						let offset = parent.child_index(id).unwrap().into();
						parent.insert(offset, median, Some(right_id));

						// new address.
						if addr.node == id {
							match addr.offset.partial_cmp(&median_offset) {
								Some(std::cmp::Ordering::Equal) => {
									addr = Address {
										node: parent_id,
										offset,
									}
								}
								Some(std::cmp::Ordering::Greater) => {
									addr = Address {
										node: right_id,
										offset: (addr.offset.unwrap() - median_offset - 1).into(),
									}
								}
								_ => (),
							}
						} else if addr.node == parent_id && addr.offset >= offset {
							addr.offset.incr()
						}

						id = parent_id;
						balance = parent.balance()
					}
					None => {
						let left_id = id;
						let new_root = Node::binary(None, left_id, median, right_id);
						let root_id = tree.insert_node(new_root);

						root = Some(root_id);
						tree.get_mut(left_id).set_parent(Some(root_id));
						tree.get_mut(right_id).set_parent(Some(root_id));

						// new address.
						if addr.node == id {
							match addr.offset.partial_cmp(&median_offset) {
								Some(std::cmp::Ordering::Equal) => {
									addr = Address {
										node: root_id,
										offset: 0.into(),
									}
								}
								Some(std::cmp::Ordering::Greater) => {
									addr = Address {
										node: right_id,
										offset: (addr.offset.unwrap() - median_offset - 1).into(),
									}
								}
								_ => (),
							}
						}

						break Some(addr);
					}
				};
			}
			Balance::Underflow(is_empty) => {
				match tree.get(id).parent() {
					Some(parent_id) => {
						let index = tree.get(parent_id).child_index(id).unwrap();
						// An underflow append in the child node.
						// First we try to rebalance the tree by rotation.
						if try_rotate_left(tree, parent_id, index, &mut addr)
							|| try_rotate_right(tree, parent_id, index, &mut addr)
						{
							break Some(addr);
						} else {
							// Rotation didn't work.
							// This means that all existing child sibling have enough few elements to be merged with this child.
							let (new_balance, new_addr) = merge(tree, parent_id, index, addr);
							balance = new_balance;
							addr = new_addr;
							// The `merge` function returns the current balance of the parent node,
							// since it may underflow after the merging operation.
							id = parent_id
						}
					}
					None => {
						// if root is empty.
						let addr = if is_empty {
							root = tree.get(id).child_id_opt(0);

							let addr = match root {
								Some(root) => {
									let root_node = tree.get_mut(root);
									root_node.set_parent(None);

									if addr.node == id {
										addr.node = root;
										addr.offset = root_node.item_count().into()
									}

									Some(addr)
								}
								None => None,
							};

							tree.release_node(id);
							addr
						} else {
							Some(addr)
						};

						break addr;
					}
				}
			}
		}
	};

	(root, addr)
}

/// Try to rotate left the node `id` to benefits the child number `deficient_child_index`.
///
/// Returns true if the rotation succeeded, of false if the target child has no right sibling,
/// or if this sibling would underflow.
#[inline]
unsafe fn try_rotate_left<T, S: Storage<T>>(
	tree: &mut S,
	id: S::Node,
	deficient_child_index: usize,
	addr: &mut Address<S::Node>,
) -> bool {
	let pivot_offset = deficient_child_index.into();
	let right_sibling_index = deficient_child_index + 1;
	let (right_sibling_id, deficient_child_id) = {
		let node = tree.get(id);

		if right_sibling_index >= node.child_count() {
			return false; // no right sibling
		}

		(
			node.child_id(right_sibling_index),
			node.child_id(deficient_child_index),
		)
	};

	match tree.get_mut(right_sibling_id).pop_left() {
		Ok((mut value, opt_child_id)) => {
			std::mem::swap(&mut value, tree.get_mut(id).item_mut(pivot_offset).unwrap());
			let left_offset = tree
				.get_mut(deficient_child_id)
				.push_right(value, opt_child_id);

			// update opt_child's parent
			if let Some(child_id) = opt_child_id {
				tree.get_mut(child_id).set_parent(Some(deficient_child_id))
			}

			// update address.
			if addr.node == right_sibling_id {
				// addressed item is in the right node.
				if addr.offset == 0 {
					// addressed item is moving to pivot.
					addr.node = id;
					addr.offset = pivot_offset;
				} else {
					// addressed item stays on right.
					addr.offset.decr();
				}
			} else if addr.node == id {
				// addressed item is in the parent node.
				if addr.offset == pivot_offset {
					// addressed item is the pivot, moving to the left (deficient) node.
					addr.node = deficient_child_id;
					addr.offset = left_offset;
				}
			}

			true // rotation succeeded
		}
		Err(WouldUnderflow) => false, // the right sibling would underflow.
	}
}

/// Try to rotate right the node `id` to benefits the child number `deficient_child_index`.
///
/// Returns true if the rotation succeeded, of false if the target child has no left sibling,
/// or if this sibling would underflow.
#[inline]
unsafe fn try_rotate_right<T, S: Storage<T>>(
	tree: &mut S,
	id: S::Node,
	deficient_child_index: usize,
	addr: &mut Address<S::Node>,
) -> bool {
	if deficient_child_index > 0 {
		let left_sibling_index = deficient_child_index - 1;
		let pivot_offset = left_sibling_index.into();
		let (left_sibling_id, deficient_child_id) = {
			let node = tree.get(id);
			(
				node.child_id(left_sibling_index),
				node.child_id(deficient_child_index),
			)
		};
		match tree.get_mut(left_sibling_id).pop_right() {
			Ok((left_offset, mut value, opt_child_id)) => {
				std::mem::swap(&mut value, tree.get_mut(id).item_mut(pivot_offset).unwrap());
				tree.get_mut(deficient_child_id)
					.push_left(value, opt_child_id);

				// update opt_child's parent
				if let Some(child_id) = opt_child_id {
					tree.get_mut(child_id).set_parent(Some(deficient_child_id))
				}

				// update address.
				if addr.node == deficient_child_id {
					// addressed item is in the right (deficient) node.
					addr.offset.incr();
				} else if addr.node == left_sibling_id {
					// addressed item is in the left node.
					if addr.offset == left_offset {
						// addressed item is moving to pivot.
						addr.node = id;
						addr.offset = pivot_offset;
					}
				} else if addr.node == id {
					// addressed item is in the parent node.
					if addr.offset == pivot_offset {
						// addressed item is the pivot, moving to the left (deficient) node.
						addr.node = deficient_child_id;
						addr.offset = 0.into();
					}
				}

				true // rotation succeeded
			}
			Err(WouldUnderflow) => false, // the left sibling would underflow.
		}
	} else {
		false // no left sibling.
	}
}

/// Merge the child `deficient_child_index` in node `id` with one of its direct sibling.
#[inline]
unsafe fn merge<T, S: Storage<T>>(
	tree: &mut S,
	id: S::Node,
	deficient_child_index: usize,
	mut addr: Address<S::Node>,
) -> (Balance, Address<S::Node>) {
	let (offset, left_id, right_id, separator, balance) = if deficient_child_index > 0 {
		// merge with left sibling
		tree.get_mut(id).merge(deficient_child_index - 1)
	} else {
		// merge with right sibling
		tree.get_mut(id).merge(deficient_child_index)
	};

	// update children's parent.
	let right_node = tree.release_node(right_id);
	for right_child_id in right_node.children() {
		tree.get_mut(right_child_id).set_parent(Some(left_id));
	}

	// actually merge.
	let left_offset = tree.get_mut(left_id).append(separator, right_node);

	// update addr.
	if addr.node == id {
		match addr.offset.partial_cmp(&offset) {
			Some(Ordering::Equal) => {
				addr.node = left_id;
				addr.offset = left_offset
			}
			Some(Ordering::Greater) => addr.offset.decr(),
			_ => (),
		}
	} else if addr.node == right_id {
		addr.node = left_id;
		addr.offset = (addr.offset.unwrap() + left_offset.unwrap() + 1).into();
	}

	(balance, addr)
}
