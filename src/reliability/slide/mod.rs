use delegate::delegate;
use std::{convert::identity, mem::MaybeUninit};

mod iter;
mod traits;

pub use iter::{SlideIter, SlideIterMut, TrackingSlideIter, TrackingSlideIterMut};

/// A sliding window with maximum size into a sequence of elements.
/// Can be used to keep track of N latest elements efficiently, without additional allocations.
///
/// Note that by itself `Slide` can be counter-intuitive, as indexing is relative to the start of
/// the `Slide` and not the collection it's sliding over. Consider using [`TrackingSlide`] if such
/// behavior is desired.
pub struct Slide<T, const N: usize> {
	length: usize,
	start: usize,
	items: [MaybeUninit<T>; N],
}

// Inspired by https://github.com/andreacorbellini/rust-circular-buffer
impl<T, const N: usize> Slide<T, N> {
	pub const MAX_LENGTH: usize = N;

	/// Construct a new circular buffer.
	#[inline]
	#[must_use]
	pub const fn new() -> Self {
		Self {
			length: 0,
			start: 0,
			items: [const { MaybeUninit::uninit() }; N],
		}
	}

	#[must_use]
	pub fn boxed() -> Box<Self> {
		let mut uninit: Box<MaybeUninit<Self>> = Box::new_uninit();
		let ptr = uninit.as_mut_ptr();
		// SAFETY: only initialize necessary fields
		unsafe {
			(*ptr).length = 0;
			(*ptr).start = 0;
			uninit.assume_init()
		}
	}

	/// Get the number of elements
	#[inline]
	pub const fn len(&self) -> usize {
		self.length
	}

	#[inline]
	pub const fn is_empty(&self) -> bool {
		self.length == 0
	}

	#[inline]
	pub const fn is_full(&self) -> bool {
		self.length == N
	}

	/// Get n-th oldest element from the slide.
	///
	/// Note that elements are overwritten when a new one is pushed to a full slide.
	///
	/// # Example
	/// ```
	/// let mut slide = Slide::<2, _, _>::new();
	/// slide.push(1);
	/// slide.push(2);
	/// slide.push(3);
	/// assert_eq!(slide.get_from_end(0), 2);
	/// assert_eq!(slide.get_from_end(1), 3);
	/// ```
	pub fn get(&self, index: usize) -> Option<&T> {
		debug_assert!(index < N, "supplied index is out-of-bounds");

		let index = if self.is_full() {
			(self.start + index) % N
		} else {
			// start is guaranteed to be 0 before the buffer is full
			if index >= self.length {
				return None;
			}
			self.length - index
		};
		// SAFETY: the index is guaranteed to be within valid range
		let item = unsafe { self.items[index].assume_init_ref() };
		Some(item)
	}

	/// Get a mutable reference to n-th oldest element in the slide.
	pub fn get_mut<'a: 'b, 'b>(&'a mut self, index: usize) -> Option<&'b mut T> {
		debug_assert!(index < N, "supplied index is out-of-bounds");

		let index = if self.is_full() {
			(self.start + index) % N
		} else {
			// start is guaranteed to be 0 before the buffer is full
			if index >= self.length {
				return None;
			}
			index
		};
		// SAFETY: the index is guaranteed to be within valid range
		let item = unsafe { self.items[index as usize].assume_init_mut() };
		Some(item)
	}

	#[inline]
	pub fn oldest(&self) -> Option<&T> {
		self.get(0)
	}

	#[inline]
	pub fn oldest_mut(&mut self) -> Option<&mut T> {
		self.get_mut(0)
	}

	/// Append an element to the slide, possibly evicting the oldest element.
	///
	/// Returns `Some(evicted)` if an eviction occurred (the slide was full)
	pub fn push(&mut self, item: T) -> Option<T> {
		if self.is_full() {
			// front is guaranteed to be present, as the buffer is full
			let evicted = std::mem::replace(unsafe { self.front_mut_unchecked() }, item);
			self.inc_start();
			Some(evicted)
		} else {
			unsafe { self.items.get_unchecked_mut(self.length).write(item) };
			self.length += 1;
			None
		}
	}

	/// Get an iterator over the elements in the slide in the order of insertion.
	/// Note that if the order is irrelevant - prefer using [`.fast_iter()`](Self::fast_iter) instead!
	#[inline]
	pub fn ordered_iter<'a>(&'a self) -> SlideIter<'a, T, N> {
		SlideIter::new(self)
	}

	/// Get a cache-friendly iterator over the elements in the slide. Note that iteration order is
	/// NOT guaranteed to be the same as insertion order.
	///
	/// If the order is relevant - use [`.ordered_iter()`](Self::ordered_iter) instead!
	#[inline]
	pub fn fast_iter<'a>(&'a self) -> std::slice::Iter<'a, T> {
		// SAFETY: the items are populated up to length, even if they were overwritten
		let slice = unsafe { self.items[0..self.len()].assume_init_ref() };
		slice.iter()
	}

	/// Get an iterator over the elements in the slide in the order of insertion.
	/// Note that if the order is irrelevant - prefer using [`.fast_iter_mut()`](Self::fast_iter_mut) instead!
	#[inline]
	pub fn ordered_iter_mut<'a>(&'a mut self) -> SlideIterMut<'a, T, N> {
		SlideIterMut::new(self)
	}

	/// Get a cache-friendly iterator over the elements in the slide. Note that iteration order is
	/// NOT guaranteed to be the same as insertion order.
	///
	/// If the order is relevant - use [`.ordered_iter_mut()`](Self::ordered_iter_mut) instead!
	#[inline]
	pub fn fast_iter_mut<'a>(&'a mut self) -> std::slice::IterMut<'a, T> {
		let range = 0..self.len();
		// SAFETY: the items are populated up to length, even if they were overwritten
		let slice = unsafe { self.items[range].assume_init_mut() };
		slice.iter_mut()
	}

	/// Map all the elements in the slide, returning a new slide.
	#[must_use]
	pub fn map<F, A>(self, mut f: F) -> Slide<A, N>
	where
		F: FnMut(T) -> A,
	{
		let mut new_items: [MaybeUninit<A>; N] = [const { MaybeUninit::uninit() }; N];

		for i in 0..self.length {
			// SAFETY: elements at 0..length are initialized
			let item = unsafe { self.items[i].assume_init_read() };
			new_items[i].write(f(item));
		}

		Slide {
			length: self.length,
			start: self.start,
			items: new_items,
		}
	}

	/// Map all elements without moving through the stack — use when the slide is boxed.
	#[must_use]
	pub fn map_boxed<F, A>(self: Box<Self>, mut f: F) -> Box<Slide<A, N>>
	where
		F: FnMut(T) -> A,
	{
		let length = self.length;
		let start = self.start;

		let mut dst: Box<MaybeUninit<Slide<A, N>>> = Box::new_uninit();
		// SAFETY: MaybeUninit<T> has same layout as T; we initialise every field before assume_init
		let dst_ptr = dst.as_mut_ptr();
		unsafe {
			(*dst_ptr).length = length;
			(*dst_ptr).start = start;
			for i in 0..length {
				let item = self.items[i].assume_init_read();
				(*dst_ptr).items[i].write(f(item));
			}
			dst.assume_init()
		}
	}
}

/// Numeric trait for tracking a sequence.
pub trait Track: PartialOrd {
	/// Increment by 1 step
	fn inc(&mut self);
	/// Get a relative index, comparing to other
	fn relative_to(&self, other: &Self) -> usize;
}

/// A version of [`Slide`] that keeps track of the number of pushes, allowing more intuitive indexing.
pub struct TrackingSlide<T, I: Track, const N: usize> {
	slide: Slide<T, N>,
	first: I,
}

impl<T, I: Track, const N: usize> TrackingSlide<T, I, N> {
	pub const MAX_LENGTH: usize = N;

	#[inline]
	#[must_use]
	pub const fn new(first: I) -> Self {
		Self {
			slide: Slide::new(),
			first,
		}
	}

	#[must_use]
	pub fn boxed(first: I) -> Box<Self> {
		let mut uninit: Box<MaybeUninit<Self>> = Box::new_uninit();
		let ptr = uninit.as_mut_ptr();
		// SAFETY: only initialize necessary fields
		unsafe {
			(*ptr).slide.length = 0;
			(*ptr).slide.start = 0;
			(*ptr).first = first;
			uninit.assume_init()
		}
	}

	/// Get the element at the provided index if it's within the `Slide`
	pub fn get(&self, index: &I) -> Option<&T> {
		if *index < self.first {
			return None;
		}
		let index = index.relative_to(&self.first);
		if index >= self.len() {
			return None;
		}
		self.slide.get(index)
	}

	pub fn get_mut<'a: 'b, 'b>(&'a mut self, index: &I) -> Option<&'b mut T> {
		if *index < self.first {
			return None;
		}
		let index = index.relative_to(&self.first);
		if index >= self.len() {
			return None;
		}
		self.slide.get_mut(index)
	}

	/// Append an element to the slide, possibly evicting the oldest element.
	///
	/// Returns `Some(evicted)` if an eviction occurred (the slide was full).
	#[inline]
	pub fn push(&mut self, item: T) -> Option<T> {
		if self.is_full() {
			self.first.inc();
		}
		self.slide.push(item)
	}

	/// Map all the elements in the slide to a different type.
	///
	/// Note that the tracking stays the same.
	#[inline]
	#[must_use]
	pub fn map_elems<F, A>(self, f: F) -> TrackingSlide<A, I, N>
	where
		F: FnMut(T) -> A,
	{
		TrackingSlide {
			slide: self.slide.map(f),
			first: self.first,
		}
	}

	/// Map all the elements in the slide to a different type, as well as the index used for tracking.
	#[inline]
	#[must_use]
	pub fn map<FE, FI, NT, NI>(self, elements: FE, index: FI) -> TrackingSlide<NT, NI, N>
	where
		NI: Track,
		FE: FnMut(T) -> NT,
		FI: FnOnce(I) -> NI,
	{
		TrackingSlide {
			slide: self.slide.map(elements),
			first: index(self.first),
		}
	}

	/// Map elements of the slide as well as its index used for tracking without unnecessary copies to stack
	pub fn map_boxed<FE, FI, NT, NI>(
		self: Box<Self>,
		mut elements: FE,
		index: FI,
	) -> Box<TrackingSlide<NT, NI, N>>
	where
		NI: Track,
		FE: FnMut(T) -> NT,
		FI: FnOnce(I) -> NI,
	{
		let length = self.slide.length;
		let start = self.slide.start;

		let mut dst: Box<MaybeUninit<TrackingSlide<NT, NI, N>>> = Box::new_uninit();
		// SAFETY: MaybeUninit<T> has same layout as T; we initialise every field before assume_init
		let dst_ptr = dst.as_mut_ptr();
		unsafe {
			(*dst_ptr).slide.length = length;
			(*dst_ptr).slide.start = start;
			(*dst_ptr).first = index(self.first);
			for i in 0..length {
				let item = self.slide.items[i].assume_init_read();
				(*dst_ptr).slide.items[i].write(elements(item));
			}
			dst.assume_init()
		}
	}

	pub fn map_boxed_elems<FE, NT>(self: Box<Self>, f: FE) -> Box<TrackingSlide<NT, I, N>>
	where
		FE: FnMut(T) -> NT,
	{
		self.map_boxed(f, identity)
	}

	delegate! {
		to self.slide {
			pub const fn len(&self) -> usize;
			pub const fn is_full(&self) -> bool;
			pub const fn is_empty(&self) -> bool;
			pub fn oldest(&self) -> Option<&T>;
			pub fn oldest_mut(&mut self) -> Option<&mut T>;
			/// Get a cache-friendly iterator over the elements in the slide. Note that iteration order is
			/// NOT guaranteed to be the same as insertion order.
			///
			/// If the order is relevant - use [`.iter()`](Self::iter) instead!
			pub fn fast_iter<'a>(&'a self) -> std::slice::Iter<'a, T>;
			/// Get a cache-friendly iterator over the elements in the slide. Note that iteration order is
			/// NOT guaranteed to be the same as insertion order.
			///
			/// If the order is relevant - use [`.iter_mut()`](Self::iter_mut) instead!
			pub fn fast_iter_mut<'a>(&'a mut self) -> std::slice::IterMut<'a, T>;
		}
	}
}

impl<T, I: Track + Copy, const N: usize> TrackingSlide<T, I, N> {
	/// Get an iterator over the (push-index, &item).
	/// Note that if the order is irrelevant prefer [`.fast_iter()`](Self::fast_iter).
	pub fn iter<'a>(&'a self) -> TrackingSlideIter<'a, T, I, N> {
		TrackingSlideIter::new(self)
	}

	/// Get an iterator over the (push-index, &mut item).
	/// Note that if the order is irrelevant prefer [`.fast_iter()`](Self::fast_iter).
	pub fn iter_mut<'a>(&'a mut self) -> TrackingSlideIterMut<'a, T, I, N> {
		TrackingSlideIterMut::new(self)
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn test_slide() {
		let mut slide: Slide<u32, 2> = Slide::new();

		slide.push(1);
		slide.push(2);
		slide.push(3);

		// element 1 is evicted on 3rd push, since the slide is only 2 elements long
		assert_eq!(*slide.get(0).unwrap(), 2);
		assert_eq!(*slide.get(1).unwrap(), 3);

		assert_eq!(slide[0], 2);
		assert_eq!(slide[1], 3);
	}

	#[test]
	fn test_iter() {
		let mut slide: TrackingSlide<u32, u8, 4> = TrackingSlide::new(0);

		slide.extend(1..=8);

		assert_eq!(slide.get(&0), None);
		assert_eq!(slide[4], 5);

		let values: Vec<(u8, u32)> = slide.iter().map(|(i, v)| (i, *v)).collect();
		assert_eq!(values, [(4, 5), (5, 6), (6, 7), (7, 8)]);
	}
}

// Helper functions
impl<T, const N: usize> Slide<T, N> {
	#[inline]
	const fn inc_start(&mut self) {
		self.start = (self.start + 1) % N;
	}

	#[inline]
	unsafe fn front_mut_unchecked(&mut self) -> &mut T {
		unsafe {
			let uninit = self.items.get_unchecked_mut(self.start);
			uninit.assume_init_mut()
		}
	}
}
