//! Trait implementations for FrameBuffer

use super::{super::SeqId, Slide, Track, TrackingSlide};
use std::{
	borrow::{Borrow, BorrowMut},
	fmt,
	mem::MaybeUninit,
	ops,
};

/// Implement [`Track`] for a numeric type.
macro_rules! impl_track {
	($($t:ty),+ $(,)?) => {
		$(impl Track for $t {
			#[inline]
			fn inc(&mut self) { *self += 1; }
			#[inline]
			fn relative_to(&self, other: &Self) -> usize { (*self - *other) as usize }
		})+
	};
}

impl_track! { u8, u16, u32, u64, usize }

impl Track for SeqId {
	#[inline]
	fn inc(&mut self) {
		self.inc()
	}

	#[inline]
	fn relative_to(&self, other: &Self) -> usize {
		(*self - *other) as usize
	}
}

impl<const N: usize, T> Default for Slide<T, N> {
	#[inline]
	fn default() -> Self {
		Self::new()
	}
}
impl<const N: usize, T, I: Track + Default> Default for TrackingSlide<T, I, N> {
	#[inline]
	fn default() -> Self {
		Self::new(I::default())
	}
}

impl<const N: usize, T> ops::Index<usize> for Slide<T, N> {
	type Output = T;
	#[inline]
	fn index(&self, index: usize) -> &T {
		self.get(index).expect("index out-of-bounds")
	}
}
impl<const N: usize, T, I: Track> ops::Index<&I> for TrackingSlide<T, I, N> {
	type Output = T;
	#[inline]
	fn index(&self, index: &I) -> &T {
		self.get(index).expect("index out-of-bounds")
	}
}
impl<const N: usize, T, I: Track + Copy> ops::Index<I> for TrackingSlide<T, I, N> {
	type Output = T;
	#[inline]
	fn index(&self, index: I) -> &T {
		self.get(&index).expect("index out-of-bounds")
	}
}

impl<const N: usize, T> ops::IndexMut<usize> for Slide<T, N> {
	#[inline]
	fn index_mut(&mut self, index: usize) -> &mut T {
		self.get_mut(index).expect("index out-of-bounds")
	}
}
impl<const N: usize, T, I: Track> ops::IndexMut<&I> for TrackingSlide<T, I, N> {
	#[inline]
	fn index_mut(&mut self, index: &I) -> &mut T {
		self.get_mut(index).expect("index out-of-bounds")
	}
}
impl<const N: usize, T, I: Track + Copy> ops::IndexMut<I> for TrackingSlide<T, I, N> {
	#[inline]
	fn index_mut(&mut self, index: I) -> &mut T {
		self.get_mut(&index).expect("index out-of-bounds")
	}
}

impl<const N: usize, T> Drop for Slide<T, N> {
	fn drop(&mut self) {
		let range = 0..self.len();
		let slice = &mut self.items[range];
		unsafe { std::ptr::drop_in_place(slice) }
	}
}

impl<const N: usize, T: fmt::Debug> fmt::Debug for Slide<T, N> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("FrameBuffer")
			.field("length", &self.length)
			.field("start", &self.start)
			.field("items", &self.items)
			.finish()
	}
}
impl<const N: usize, T: fmt::Debug, I: Track + fmt::Debug> fmt::Debug for TrackingSlide<T, I, N> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("TrackingSlide")
			.field("slide", &self.slide)
			.field("first", &self.first)
			.finish()
	}
}

impl<const N: usize, T> Clone for Slide<T, N>
where
	MaybeUninit<T>: Clone,
{
	fn clone(&self) -> Self {
		Self {
			length: self.length,
			start: self.start,
			items: self.items.clone(),
		}
	}
}
impl<const N: usize, T, I: Track + Clone> Clone for TrackingSlide<T, I, N>
where
	Slide<T, N>: Clone,
{
	fn clone(&self) -> Self {
		Self {
			slide: self.slide.clone(),
			first: self.first.clone(),
		}
	}
}

impl<const N: usize, T> Extend<T> for Slide<T, N> {
	fn extend<TI: IntoIterator<Item = T>>(&mut self, iter: TI) {
		let mut iter = iter.into_iter();
		let (mut elems, _) = iter.size_hint();
		while elems > Self::MAX_LENGTH {
			iter.next();
			elems -= 1;
		}

		for e in iter {
			self.push(e);
		}
	}
}
impl<const N: usize, T, I: Track> Extend<T> for TrackingSlide<T, I, N> {
	#[inline]
	fn extend<TI: IntoIterator<Item = T>>(&mut self, iter: TI) {
		let mut iter = iter.into_iter();
		let (mut elems, _) = iter.size_hint();
		while elems > Self::MAX_LENGTH {
			iter.next();
			elems -= 1;
			self.first.inc();
		}

		for e in iter {
			self.push(e);
		}
	}
}

impl<'a, const N: usize, T: Copy> Extend<&'a T> for Slide<T, N> {
	fn extend<TI: IntoIterator<Item = &'a T>>(&mut self, iter: TI) {
		let mut iter = iter.into_iter();
		let (mut elems, _) = iter.size_hint();
		while elems > Self::MAX_LENGTH {
			iter.next();
			elems -= 1;
		}

		iter.for_each(|e| {
			self.push(*e);
		});
	}
}
impl<'a, const N: usize, T, I: Track> Extend<&'a T> for TrackingSlide<T, I, N>
where
	Slide<T, N>: Extend<&'a T>,
{
	#[inline]
	fn extend<TI: IntoIterator<Item = &'a T>>(&mut self, iter: TI) {
		self.slide.extend(iter)
	}
}

impl<const N: usize, T> Borrow<[T]> for Slide<T, N> {
	#[inline]
	fn borrow(&self) -> &[T] {
		self.as_ref()
	}
}
impl<const N: usize, T, I: Track> Borrow<[T]> for TrackingSlide<T, I, N> {
	#[inline]
	fn borrow(&self) -> &[T] {
		self.slide.borrow()
	}
}
impl<const N: usize, T, I: Track> Borrow<Slide<T, N>> for TrackingSlide<T, I, N> {
	#[inline]
	fn borrow(&self) -> &Slide<T, N> {
		&self.slide
	}
}

impl<const N: usize, T> BorrowMut<[T]> for Slide<T, N> {
	#[inline]
	fn borrow_mut(&mut self) -> &mut [T] {
		self.as_mut()
	}
}
impl<const N: usize, T, I: Track> BorrowMut<[T]> for TrackingSlide<T, I, N> {
	#[inline]
	fn borrow_mut(&mut self) -> &mut [T] {
		self.slide.borrow_mut()
	}
}
impl<const N: usize, T, I: Track> BorrowMut<Slide<T, N>> for TrackingSlide<T, I, N> {
	#[inline]
	fn borrow_mut(&mut self) -> &mut Slide<T, N> {
		&mut self.slide
	}
}

impl<const N: usize, T> AsRef<[T]> for Slide<T, N> {
	#[inline]
	fn as_ref(&self) -> &[T] {
		let slice = &self.items[0..self.len()];
		unsafe { slice.assume_init_ref() }
	}
}
impl<const N: usize, T, I: Track> AsRef<[T]> for TrackingSlide<T, I, N> {
	#[inline]
	fn as_ref(&self) -> &[T] {
		self.slide.as_ref()
	}
}
impl<const N: usize, T, I: Track> AsRef<Slide<T, N>> for TrackingSlide<T, I, N> {
	#[inline]
	fn as_ref(&self) -> &Slide<T, N> {
		&self.slide
	}
}

impl<const N: usize, T> AsMut<[T]> for Slide<T, N> {
	#[inline]
	fn as_mut(&mut self) -> &mut [T] {
		let range = 0..self.len();
		let slice = &mut self.items[range];
		unsafe { slice.assume_init_mut() }
	}
}
impl<const N: usize, T, I: Track> AsMut<[T]> for TrackingSlide<T, I, N> {
	#[inline]
	fn as_mut(&mut self) -> &mut [T] {
		self.slide.as_mut()
	}
}
