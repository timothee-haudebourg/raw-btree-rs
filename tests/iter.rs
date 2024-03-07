use std::{cell::Cell, rc::Rc};

use raw_btree::{Item, RawBTree};

#[test]
pub fn iter() {
	let mut map: RawBTree<Item<i32, i32>> = RawBTree::new();
	for i in 0..10 {
		map.insert(Item::cmp, Item::new(i, i));
	}

	let mut i = 0;
	for item in &map {
		assert_eq!(item.key, i);
		i += 1;
	}

	assert_eq!(i, 10)
}

#[test]
pub fn into_iter() {
	struct Element {
		/// Drop counter.
		counter: Rc<Cell<usize>>,
		value: i32,
	}

	impl Element {
		pub fn new(counter: &Rc<Cell<usize>>, value: i32) -> Self {
			Element {
				counter: counter.clone(),
				value,
			}
		}

		pub fn inner(&self) -> i32 {
			self.value
		}
	}

	impl Drop for Element {
		fn drop(&mut self) {
			let c = self.counter.get();
			self.counter.set(c + 1);
		}
	}

	let counter = Rc::new(Cell::new(0));
	let mut map: RawBTree<_> = RawBTree::new();
	for i in 0..100 {
		map.insert(Item::cmp, Item::new(i, Element::new(&counter, i)));
	}

	for item in map {
		assert_eq!(item.key, item.value.inner());
	}

	assert_eq!(counter.get(), 100);
}

#[test]
pub fn into_iter_rev() {
	struct Element {
		/// Drop counter.
		counter: Rc<Cell<usize>>,
		value: i32,
	}

	impl Element {
		pub fn new(counter: &Rc<Cell<usize>>, value: i32) -> Self {
			Element {
				counter: counter.clone(),
				value,
			}
		}

		pub fn inner(&self) -> i32 {
			self.value
		}
	}

	impl Drop for Element {
		fn drop(&mut self) {
			let c = self.counter.get();
			self.counter.set(c + 1);
		}
	}

	let counter = Rc::new(Cell::new(0));
	let mut map: RawBTree<_> = RawBTree::new();
	for i in 0..100 {
		map.insert(Item::cmp, Item::new(i, Element::new(&counter, i)));
	}

	for item in map.into_iter().rev() {
		assert_eq!(item.key, item.value.inner());
	}

	assert_eq!(counter.get(), 100);
}

#[test]
pub fn into_iter_both_ends1() {
	struct Element {
		/// Drop counter.
		counter: Rc<Cell<usize>>,
		value: i32,
	}

	impl Element {
		pub fn new(counter: &Rc<Cell<usize>>, value: i32) -> Self {
			Element {
				counter: counter.clone(),
				value,
			}
		}

		pub fn inner(&self) -> i32 {
			self.value
		}
	}

	impl Drop for Element {
		fn drop(&mut self) {
			let c = self.counter.get();
			self.counter.set(c + 1);
		}
	}

	let counter = Rc::new(Cell::new(0));
	let mut map: RawBTree<_> = RawBTree::new();
	for i in 0..100 {
		map.insert(Item::cmp, Item::new(i, Element::new(&counter, i)));
	}

	let mut it = map.into_iter();
	while let Some(item) = it.next() {
		assert_eq!(item.key, item.value.inner());

		let item = it.next_back().unwrap();
		assert_eq!(item.key, item.value.inner());
	}

	assert_eq!(counter.get(), 100);
}

#[test]
pub fn into_iter_both_ends2() {
	struct Element {
		/// Drop counter.
		counter: Rc<Cell<usize>>,
		value: i32,
	}

	impl Element {
		pub fn new(counter: &Rc<Cell<usize>>, value: i32) -> Self {
			Element {
				counter: counter.clone(),
				value,
			}
		}

		pub fn inner(&self) -> i32 {
			self.value
		}
	}

	impl Drop for Element {
		fn drop(&mut self) {
			let c = self.counter.get();
			self.counter.set(c + 1);
		}
	}

	let counter = Rc::new(Cell::new(0));
	let mut map: RawBTree<_> = RawBTree::new();
	for i in 0..100 {
		map.insert(Item::cmp, Item::new(i, Element::new(&counter, i)));
	}

	let mut it = map.into_iter();
	while let Some(item) = it.next_back() {
		assert_eq!(item.key, item.value.inner());

		let item = it.next().unwrap();
		assert_eq!(item.key, item.value.inner());
	}

	assert_eq!(counter.get(), 100);
}
