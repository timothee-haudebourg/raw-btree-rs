mod array;
use std::cmp::Ordering;

pub use array::Array;

/// Search in `sorted_slice` for the item with the nearest key smaller or equal to the given one.
///
/// `sorted_slice` is assumed to be sorted.
#[inline]
pub fn binary_search_min<T, Q: ?Sized>(
	cmp: impl Fn(&T, &Q) -> Ordering,
	sorted_slice: &[T],
	key: &Q,
) -> Option<(usize, bool)> {
	let i_ord = cmp(&sorted_slice[0], key);
	if sorted_slice.is_empty() || i_ord.is_gt() {
		None
	} else {
		let mut i = 0;
		let mut j = sorted_slice.len() - 1;

		let j_ord = cmp(&sorted_slice[j], key);
		if j_ord.is_le() {
			return Some((j, j_ord.is_eq()));
		}

		// invariants:
		// sorted_slice[i].key <= key
		// sorted_slice[j].key > key
		// j > i

		let mut eq = i_ord.is_eq();
		while !eq && j - i > 1 {
			let k = (i + j) / 2;

			let k_ord = cmp(&sorted_slice[k], key);
			if k_ord.is_gt() {
				j = k;
			// sorted_slice[k].key > key --> sorted_slice[j] > key
			} else {
				i = k;
				eq = k_ord.is_eq();
				// sorted_slice[k].key <= key --> sorted_slice[i] <= key
			}
		}

		Some((i, eq))
	}
}
