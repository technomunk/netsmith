use super::{Slide, Track, TrackingSlide};

pub struct SlideIter<'a, T: 'a, const N: usize> {
	slide: &'a Slide<T, N>,
	index: usize,
}
impl<'a, T: 'a, const N: usize> SlideIter<'a, T, N> {
	#[inline]
	#[must_use]
	pub const fn new(slide: &'a Slide<T, N>) -> Self {
		Self { slide, index: 0 }
	}
}
impl<'a, T: 'a, const N: usize> Iterator for SlideIter<'a, T, N> {
	type Item = &'a T;

	fn next(&mut self) -> Option<Self::Item> {
		if self.index < self.slide.len() {
			let result = &self.slide[self.index];
			self.index += 1;
			Some(result)
		} else {
			None
		}
	}
}
impl<'a, T: 'a, const N: usize> ExactSizeIterator for SlideIter<'a, T, N> {}

pub struct SlideIterMut<'a, T: 'a, const N: usize> {
	slide: &'a mut Slide<T, N>,
	index: usize,
}
impl<'a, T: 'a, const N: usize> SlideIterMut<'a, T, N> {
	#[inline]
	#[must_use]
	pub const fn new(slide: &'a mut Slide<T, N>) -> Self {
		Self { slide, index: 0 }
	}
}
impl<'a, T: 'a, const N: usize> Iterator for SlideIterMut<'a, T, N> {
	type Item = &'a mut T;
	fn next(&mut self) -> Option<Self::Item> {
		if self.index < self.slide.len() {
			// SAFETY: monotonic advance means no two &mut to same element coexist
			let result = unsafe { &mut *(&mut self.slide[self.index] as *mut T) };
			self.index += 1;
			Some(result)
		} else {
			None
		}
	}
	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		let size = self.slide.len() - self.index;
		(size, Some(size))
	}
}
impl<'a, T: 'a, const N: usize> ExactSizeIterator for SlideIterMut<'a, T, N> {}

pub struct TrackingSlideIter<'a, T: 'a, I: 'a + Track + Copy, const N: usize> {
	slide: &'a TrackingSlide<T, I, N>,
	index: I,
}
impl<'a, T: 'a, I: 'a + Track + Copy, const N: usize> TrackingSlideIter<'a, T, I, N> {
	#[inline]
	#[must_use]
	pub const fn new(slide: &'a TrackingSlide<T, I, N>) -> Self {
		Self {
			slide,
			index: slide.first,
		}
	}
}
impl<'a, T: 'a, I: 'a + Track + Copy, const N: usize> Iterator for TrackingSlideIter<'a, T, I, N> {
	type Item = (I, &'a T);
	fn next(&mut self) -> Option<Self::Item> {
		let result = self.slide.get(&self.index).map(|i| (self.index, i));
		if result.is_some() {
			self.index.inc();
		}
		result
	}
	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		let size = self.slide.len() - self.index.relative_to(&self.slide.first);
		(size, Some(size))
	}
}
impl<'a, T: 'a, I: 'a + Track + Copy, const N: usize> ExactSizeIterator
	for TrackingSlideIter<'a, T, I, N>
{
}

pub struct TrackingSlideIterMut<'a, T: 'a, I: 'a + Track + Copy, const N: usize> {
	slide: &'a mut TrackingSlide<T, I, N>,
	index: I,
}
impl<'a, T: 'a, I: 'a + Track + Copy, const N: usize> TrackingSlideIterMut<'a, T, I, N> {
	#[inline]
	#[must_use]
	pub const fn new(slide: &'a mut TrackingSlide<T, I, N>) -> Self {
		Self {
			index: slide.first,
			slide,
		}
	}
}
impl<'a, T: 'a, I: 'a + Track + Copy, const N: usize> Iterator
	for TrackingSlideIterMut<'a, T, I, N>
{
	type Item = (I, &'a mut T);
	fn next(&mut self) -> Option<Self::Item> {
		let result = self.slide.get_mut(&self.index).map(|i| {
			// SAFETY: monotonic index advancement means no two &mut to same element coexist
			(self.index, unsafe { &mut *(i as *mut T) })
		});
		if result.is_some() {
			self.index.inc();
		}
		result
	}
	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		let size = self.slide.len() - self.index.relative_to(&self.slide.first);
		(size, Some(size))
	}
}
impl<'a, T: 'a, I: 'a + Track + Copy, const N: usize> ExactSizeIterator
	for TrackingSlideIterMut<'a, T, I, N>
{
}
impl<'a, T: 'a, I: 'a + Track + Copy, const N: usize> IntoIterator for &'a TrackingSlide<T, I, N> {
	type Item = (I, &'a T);
	type IntoIter = TrackingSlideIter<'a, T, I, N>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter()
	}
}
impl<'a, T: 'a, I: 'a + Track + Copy, const N: usize> IntoIterator
	for &'a mut TrackingSlide<T, I, N>
{
	type Item = (I, &'a mut T);
	type IntoIter = TrackingSlideIterMut<'a, T, I, N>;

	fn into_iter(self) -> Self::IntoIter {
		self.iter_mut()
	}
}
