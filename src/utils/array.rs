use std::{
	iter::FusedIterator,
	mem::MaybeUninit,
	ops::{Bound, Deref, DerefMut, RangeBounds},
};

pub struct Array<T, const N: usize> {
	len: usize,
	buffer: [MaybeUninit<T>; N],
}

impl<T, const N: usize> Default for Array<T, N> {
	fn default() -> Self {
		Self::new()
	}
}

impl<T, const N: usize> Array<T, N> {
	pub fn new() -> Self {
		let buffer: MaybeUninit<[MaybeUninit<T>; N]> = MaybeUninit::uninit();

		Self {
			len: 0,
			buffer: unsafe { MaybeUninit::assume_init(buffer) },
		}
	}

	pub fn into_raw_parts(mut self) -> (usize, [MaybeUninit<T>; N]) {
		let mut buffer = unsafe { MaybeUninit::<[MaybeUninit<T>; N]>::uninit().assume_init() };
		std::mem::swap(&mut buffer, &mut self.buffer);
		let len = self.len;
		std::mem::forget(self);
		(len, buffer)
	}

	pub fn len(&self) -> usize {
		self.len
	}

	pub fn is_empty(&self) -> bool {
		self.len == 0
	}

	pub fn as_slice(&self) -> &[T] {
		let slice = &self.buffer[..self.len];
		unsafe {
			// SAFETY: casting `slice` to a `*const [T]` is safe since the caller guarantees that
			// `slice` is initialized, and `MaybeUninit` is guaranteed to have the same layout as `T`.
			// The pointer obtained is valid since it refers to memory owned by `slice` which is a
			// reference and thus guaranteed to be valid for reads.
			&*(slice as *const [MaybeUninit<T>] as *const [T])
		}
	}

	pub fn as_slice_mut(&mut self) -> &mut [T] {
		let slice = &mut self.buffer[..self.len];
		unsafe {
			// SAFETY: casting `slice` to a `*const [T]` is safe since the caller guarantees that
			// `slice` is initialized, and `MaybeUninit` is guaranteed to have the same layout as `T`.
			// The pointer obtained is valid since it refers to memory owned by `slice` which is a
			// reference and thus guaranteed to be valid for reads.
			&mut *(slice as *mut [MaybeUninit<T>] as *mut [T])
		}
	}

	pub fn push(&mut self, value: T) {
		if self.len < N {
			self.buffer[self.len].write(value);
			self.len += 1
		} else {
			panic!("array is full")
		}
	}

	pub fn pop(&mut self) -> Option<T> {
		if self.len == 0 {
			None
		} else {
			self.len -= 1;
			Some(unsafe { self.buffer[self.len].assume_init_read() })
		}
	}

	pub fn insert(&mut self, i: usize, value: T) {
		if i <= self.len {
			if i < N {
				for j in (i..self.len).rev() {
					self.buffer[j + 1].write(unsafe { self.buffer[j].assume_init_read() });
				}
				self.buffer[i].write(value);
				self.len += 1;
			} else {
				panic!("array is full")
			}
		} else {
			panic!("cannot insert out of bounds")
		}
	}

	pub fn append(&mut self, other: &mut Self) {
		for t in other.drain(..) {
			self.push(t)
		}
	}

	pub fn remove(&mut self, i: usize) -> Option<T> {
		if i < self.len {
			let t = unsafe { self.buffer[i].assume_init_read() };
			for j in (i + 1)..self.len {
				self.buffer[j - 1].write(unsafe { self.buffer[j].assume_init_read() });
			}

			self.len -= 1;
			Some(t)
		} else {
			None
		}
	}

	pub fn drain(&mut self, range: impl RangeBounds<usize>) -> Drain<T, N> {
		let start = match range.start_bound() {
			Bound::Unbounded => 0,
			Bound::Included(i) => *i,
			Bound::Excluded(i) => *i + 1,
		};

		let end = match range.end_bound() {
			Bound::Unbounded => self.len,
			Bound::Included(i) => *i + 1,
			Bound::Excluded(i) => *i,
		};

		let len = self.len;

		if start < end {
			self.len -= end - start;
		}

		Drain {
			start,
			end,
			front: start,
			back: end,
			buffer: &mut self.buffer,
			len,
		}
	}

	pub fn clear(&mut self) {
		for i in 0..self.len {
			unsafe { self.buffer[i].assume_init_drop() };
		}
		self.len = 0;
	}
}

impl<T, const N: usize> Drop for Array<T, N> {
	fn drop(&mut self) {
		for i in 0..self.len {
			unsafe { self.buffer[i].assume_init_drop() };
		}
	}
}

impl<T: Clone, const N: usize> Clone for Array<T, N> {
	fn clone(&self) -> Self {
		let mut result = Array::new();

		for i in 0..self.len {
			result.buffer[i].write(unsafe { self.buffer[i].assume_init_ref() }.clone());
		}
		result.len = self.len;

		result
	}
}

impl<T, const N: usize> Deref for Array<T, N> {
	type Target = [T];

	fn deref(&self) -> &Self::Target {
		self.as_slice()
	}
}

impl<T, const N: usize> DerefMut for Array<T, N> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		self.as_slice_mut()
	}
}

impl<T, const N: usize> Extend<T> for Array<T, N> {
	fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
		for t in iter {
			self.push(t)
		}
	}
}

impl<T, const N: usize> FromIterator<T> for Array<T, N> {
	fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
		let mut result = Self::new();
		result.extend(iter);
		result
	}
}

impl<'a, T, const N: usize> IntoIterator for &'a Array<T, N> {
	type IntoIter = std::slice::Iter<'a, T>;
	type Item = &'a T;

	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}

impl<T, const N: usize> IntoIterator for Array<T, N> {
	type IntoIter = IntoIter<T, N>;
	type Item = T;

	fn into_iter(self) -> Self::IntoIter {
		let (len, buffer) = self.into_raw_parts();

		IntoIter {
			front: 0,
			back: len,
			buffer,
		}
	}
}

pub struct Drain<'a, T, const N: usize> {
	start: usize,
	end: usize,
	front: usize,
	back: usize,
	buffer: &'a mut [MaybeUninit<T>; N],
	len: usize,
}

impl<'a, T, const N: usize> Drain<'a, T, N> {
	fn shift(&mut self) {
		if self.start < self.end {
			for i in self.end..self.len {
				self.buffer[self.start].write(unsafe { self.buffer[i].assume_init_read() });
				self.start += 1;
			}

			self.end = self.start
		}
	}
}

impl<'a, T, const N: usize> Iterator for Drain<'a, T, N> {
	type Item = T;

	fn size_hint(&self) -> (usize, Option<usize>) {
		let size = if self.front < self.back {
			self.back - self.front
		} else {
			0
		};

		(size, Some(size))
	}

	fn next(&mut self) -> Option<Self::Item> {
		if self.front < self.back {
			let t = unsafe { self.buffer[self.front].assume_init_read() };
			self.front += 1;
			Some(t)
		} else {
			self.shift();
			None
		}
	}
}

impl<'a, T, const N: usize> DoubleEndedIterator for Drain<'a, T, N> {
	fn next_back(&mut self) -> Option<Self::Item> {
		if self.front < self.back {
			self.back -= 1;
			let t = unsafe { self.buffer[self.back].assume_init_read() };
			Some(t)
		} else {
			self.shift();
			None
		}
	}
}

impl<'a, T, const N: usize> FusedIterator for Drain<'a, T, N> {}
impl<'a, T, const N: usize> ExactSizeIterator for Drain<'a, T, N> {}

impl<'a, T, const N: usize> Drop for Drain<'a, T, N> {
	fn drop(&mut self) {
		let _ = self.last();
	}
}

pub struct IntoIter<T, const N: usize> {
	front: usize,
	back: usize,
	buffer: [MaybeUninit<T>; N],
}

impl<T, const N: usize> Iterator for IntoIter<T, N> {
	type Item = T;

	fn size_hint(&self) -> (usize, Option<usize>) {
		let len = self.back - self.front;
		(len, Some(len))
	}

	fn next(&mut self) -> Option<Self::Item> {
		if self.front < self.back {
			let t = unsafe { self.buffer[self.front].assume_init_read() };
			self.front += 1;
			Some(t)
		} else {
			None
		}
	}
}

impl<T, const N: usize> DoubleEndedIterator for IntoIter<T, N> {
	fn next_back(&mut self) -> Option<Self::Item> {
		if self.front < self.back {
			self.back -= 1;
			let t = unsafe { self.buffer[self.back].assume_init_read() };
			Some(t)
		} else {
			None
		}
	}
}

impl<T, const N: usize> FusedIterator for IntoIter<T, N> {}
impl<T, const N: usize> ExactSizeIterator for IntoIter<T, N> {}

impl<T, const N: usize> Drop for IntoIter<T, N> {
	fn drop(&mut self) {
		let _ = self.last();
	}
}
