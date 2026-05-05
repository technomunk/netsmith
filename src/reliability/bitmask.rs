/// Trait for efficiently tracking boolean state of a section of a sequence.
///
/// Used by the reliability layer to keep track of delivered packets while minimizing the overhead.
///
/// The library provides implementation backed by all unsigned integers and the reliability layer
/// assumes a fixed size, however does not enforce said assumption.
pub trait Bitmask: Copy {
	/// Maximum number of tracked bits in the mask.
	///
	/// The index/count supplied to all operations is guaranteed to be less than the given number.
	const MAX_BITS: u16;

	/// Create a new instance of the bitmask with none of the bits set
	fn empty() -> Self;

	/// Create a new instance of the bitmask with all the possible bits set
	fn filled() -> Self;

	// TODO/Grig: figure out index semantics
	/// Set the bit at the provided index.
	///
	/// The index counts backwards from the "newest" one, meaning bit 0 should correspond to the latest
	/// in the mask. This corresponds to the least significant one.
	fn set(&mut self, index: u16);

	/// Unset the bit at the provided index.
	///
	/// The index counts backwards from the "newest" one, meaning bit 0 should correspond to the latest
	/// in the mask. This corresponds to the least significant one.
	fn unset(&mut self, index: u16);

	/// Unset all the bits that are set in `other`.
	fn unset_every(&mut self, mask: Self);

	/// Check whether the bit at the provided index is set. The semantics of the index need to match
	/// [`.set`](Self::set).
	fn is_set(&self, index: u16) -> bool;

	/// Shift all the existing bits by a given amount towards "old" (higher index) direction.
	/// Meaning that a previous index `x` should become `x + count`.
	///
	/// ```
	/// let mask: Bitmask = ...;
	/// let before_shift = mask.is_set(2);
	/// mask.shift(3);
	/// assert_eq!(mask.is_set(2 + 3), before_shift);
	/// ```
	fn shift(&mut self, count: u16);

	/// Shift all the existing bits by a given amount towards the "new" (lower index) direction.
	///
	/// This operation should be a soft inverse of [`shift`](Self::shift), however some bits may be
	/// lost during the translation if they do not fit within the bitmask.
	fn unshift(&mut self, count: u16);

	/// Get the number of oldest (largest indices) contiguous set bits in `self`.
	fn highest_ones(&self) -> u16;

	/// Get the number of newest (lowest indices) contiguous unset bits in `self`.
	fn lowest_zeros(&self) -> u16;

	/// Get the total number of set bits in `self`
	fn count_ones(&self) -> u16;

	#[inline]
	fn is_empty(&self) -> bool {
		self.count_ones() == 0
	}
}

macro_rules! impl_bitmask {
   ($($t:ty),+ $(,)?) => {
        $(impl Bitmask for $t {
        	const MAX_BITS: u16 = <$t>::BITS as u16;
         	#[inline]
			fn empty() -> Self {
				0
			}
			#[inline]
			fn filled() -> Self {
				Self::MAX
			}
			#[inline]
			fn set(&mut self, index: u16) {
				*self |= 1 << index;
			}
			#[inline]
			fn unset(&mut self, index: u16) {
				*self &= !(1 << index);
			}
			#[inline]
			fn unset_every(&mut self, other: Self) {
				*self &= !other;
			}
			#[inline]
			fn is_set(&self, index: u16) -> bool {
				*self & (1 << index) != 0
			}
			#[inline]
			fn shift(&mut self, count: u16) {
				*self = self.wrapping_shl(count as u32);
			}
			#[inline]
			fn unshift(&mut self, count: u16) {
				*self = self.wrapping_shr(count as u32);
			}
			#[inline]
			fn highest_ones(&self) -> u16 {
				(*self).leading_ones() as u16
			}
			#[inline]
			fn lowest_zeros(&self) -> u16 {
				(*self).trailing_zeros() as u16
			}
			#[inline]
			fn count_ones(&self) -> u16 {
				(*self).count_ones() as u16
			}
        })+
    };
}

impl_bitmask! { u8, u16, u32, u64 }
