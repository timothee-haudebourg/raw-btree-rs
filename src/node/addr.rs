use super::Offset;
use std::fmt;

/// Item/entry location in a BTreeMap.
///
/// Each item in a B-Tree is addressed by a node identifier and an offset in the node.
/// We write `@id:offset` the address of the item contained in the node `id` at offset `offset`.
///
/// ```text
///                                   ┌────────────────┐
///                                   │ node 0      ┌──┼─── this item address is `@0:1`
///                                   │┌────────┐ ┌─v─┐│
///                        ┌───────── ││ item 0 │ │ 1 ││ ──────────┐
///                        │          │└────────┘ └───┘│           │
///                        │          └────────────────┘           │
///                        │                   │                   │
///               ┌─────────────────┐ ┌─────────────────┐ ┌─────────────────┐
///               │ node 1          │ │ node 2          │ │ node 3          │
///               │┌───┐ ┌───┐ ┌───┐│ │┌───┐ ┌───┐ ┌───┐│ │┌───┐ ┌───┐ ┌───┐│
///               ││ 0 │ │ 1 │ │ 2 ││ ││ 0 │ │ 1 │ │ 2 ││ ││ 0 │ │ 1 │ │ 2 ││
///               │└─^─┘ └───┘ └───┘│ │└───┘ └───┘ └─^─┘│ │└───┘ └───┘ └───┘│
///               └──┼──────────────┘ └──────────────┼──┘ └─────────────────┘
///                  └─ this item address is `@1:0`  └─ this item address is `@2:2`
/// ```
///
/// ## Validity
/// An item adress `addr` is *valid* in a given BTreeMap if it `addr.id` refers to an existing
/// node and if `addr.offset` is comprised between `-1` and the number of items in the node (included).
/// We say that `addr` is *occupied* if it points to an actual item
/// (`addr.offset` at least 0 and less than the number of items in the node).
///
/// The following diagram shows all the valid address in a B-Tree.
/// ```text
///                                             ┌───────────┐
///                                             │ node 0    │
///                                        ┌───┐│┌───┐ ┌───┐│┌───┐
///                   ┌────────────────────│-1 │││ 0 │ │ 1 │││ 2 │─────────────────┐
///                   │                    └───┘│└───┘ └───┘│└───┘                 │
///                   │                         └───────────┘                      │
///                   │                               │                            │
///          ┌─────────────────┐             ┌─────────────────┐             ┌───────────┐
///          │ node 1          │             │ node 2          │             │ node 3    │    
///     ┌───┐│┌───┐ ┌───┐ ┌───┐│┌───┐   ┌───┐│┌───┐ ┌───┐ ┌───┐│┌───┐   ┌───┐│┌───┐ ┌───┐│┌───┐
///     │-1 │││ 0 │ │ 1 │ │ 2 │││ 3 │   │-1 │││ 0 │ │ 1 │ │ 2 │││ 3 │   │-1 │││ 0 │ │ 1 │││ 2 │
///     └───┘│└───┘ └───┘ └───┘│└───┘   └───┘│└───┘ └───┘ └───┘│└───┘   └───┘│└───┘ └───┘│└───┘
///          └─────────────────┘             └─────────────────┘             └───────────┘
/// ```
/// Note how some valid addresses are outside of nodes bounds.
/// Even is thoses addresses do not refers to any items,
/// they can be useful to operate on the tree.
///
/// ### Back addresses
///
/// A "back address" is a valid address whose offset is at least `0`.
/// If `addr.offset` is equal to the number of items in the node then it doesn't actually refer
/// to an existing item in the node,
/// but it can be used to insert a new item with `BTreeExt::insert_at`.
///
/// The following diagram shows all the back addresses in a node.
/// ```text
///                        ┌───────────┄┄┄┄┄───────┐      
///                        │ node i                │       
///                        │┌───┐ ┌───┐     ┌─────┐│┌───┐  where `n` is
///                        ││ 0 │ │ 1 │ ... │ n-1 │││ n │  the number of items in the node.
///                        │└───┘ └───┘     └─────┘│└───┘  
///                        └───────────┄┄┄┄┄───────┘   
/// ```
/// Note that an address with offset `-1` is *not* a back address.
///
/// ### Front addresses
///
/// A "front address" is a valid address whose offset is less that the number of items in the node.
/// If `addr.offset` is equal to `-1`, then it doesn't actually refer to an existing item in the node.
///
/// The following diagram shows all the front addresses in a node.
/// ```text
///                             ┌───────────┄┄┄┄┄───────┐      
///                             │ node i                │       
///                        ┌───┐│┌───┐ ┌───┐     ┌─────┐│  where `n` is
///                        │-1 │││ 0 │ │ 1 │ ... │ n-1 ││  the number of items in the node.
///                        └───┘│└───┘ └───┘     └─────┘│  
///                             └───────────┄┄┄┄┄───────┘   
/// ```
/// Note that an address with offset `n` is *not* a front address.
///
/// ## Safety
/// It is not safe to use an address `addr` in which `addr.id` is not the identifier of any node
/// currently used by the tree.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Address<T> {
	/// Identifier of the node.
	pub node: T,

	/// Offset in the node.
	pub offset: Offset,
}

impl<T> Address<T> {
	/// Creates a new address from the identifier of the node and the offset in the node.
	#[inline]
	pub fn new(id: T, offset: Offset) -> Address<T> {
		Address { node: id, offset }
	}

	// /// Address in the empty tree.
	// ///
	// /// This is the unique valid address address in an ampty tree.
	// /// It is only valid in the empty tree.
	// #[inline]
	// pub fn nowhere() -> Address<T> {
	// 	Address {
	// 		id: std::usize::MAX,
	// 		offset: 0.into(),
	// 	}
	// }

	// /// Checks if the address is nowhere.
	// #[inline]
	// pub fn is_nowhere(&self) -> bool {
	// 	self.id == std::usize::MAX
	// }
}

impl<T: fmt::Display> fmt::Display for Address<T> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "@{}:{}", self.node, self.offset)
	}
}

impl<T: fmt::Debug> fmt::Debug for Address<T> {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "@{:?}:{}", self.node, self.offset)
	}
}
