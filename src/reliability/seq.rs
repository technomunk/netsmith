//! Sequential packet index details

use super::bitmask::Bitmask;
use crate::serializable;
use std::{
	cmp::Ordering,
	fmt,
	ops::{Add, Sub},
};
use thiserror::Error;

/// Packet sequence id with support for wrap-around.
///
/// Recently wrapped values are considered newer (larger) than unwrapped ones (so 1 > 65_000) holds true
#[serializable]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct SeqId(u16);

impl SeqId {
	const HALF_WAY: u16 = (u16::MAX >> 1) + 1;

	/// Construct the zero-th sequence id (same as default())
	#[inline]
	pub const fn zero() -> Self {
		Self(0)
	}

	/// Increment the sequential id, setting self to the next value.
	///
	/// equivalent to
	/// ```
	/// *x = x.next()
	/// ```
	#[inline]
	pub const fn inc(&mut self) {
		self.0 = self.0.wrapping_add(1);
	}

	/// Get the next sequential id
	#[inline]
	pub const fn next(self) -> Self {
		Self(self.0.wrapping_add(1))
	}
}

impl Ord for SeqId {
	fn cmp(&self, other: &Self) -> Ordering {
		if self == other {
			return Ordering::Equal;
		}

		if *self - *other < Self::HALF_WAY {
			Ordering::Greater
		} else {
			Ordering::Less
		}
	}
}

impl PartialOrd for SeqId {
	#[inline]
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl Add<u16> for SeqId {
	type Output = Self;

	#[inline]
	fn add(self, rhs: u16) -> Self::Output {
		Self(self.0.wrapping_add(rhs))
	}
}

impl Sub for SeqId {
	type Output = u16;

	#[inline]
	fn sub(self, rhs: Self) -> Self::Output {
		self.0.wrapping_sub(rhs.0)
	}
}

/// The provided sequence number couldn't be checked, as it's outside expected bounds
#[derive(Debug, PartialEq, Eq, Clone, Error)]
#[error("provided sequence id lies outside expected bounds")]
pub struct OutOfBoundsError;

/// Setting provided [`SeqId`] would make later acknowledgement of a preceding [`SeqId`] impossible.
#[derive(Debug, PartialEq, Eq, Clone, Error)]
#[error(
	"setting provided sequence id would make it impossible to acknowledge a preceding sequence id"
)]
pub struct HoleError;

/// Block of acknowledgements. Used to keep track of delivered packets in the reliability layer.
#[serializable]
#[derive(Debug, Clone, Copy)]
pub struct AckBlock<B: Bitmask> {
	pub index: SeqId,
	pub mask: B,
}

impl<B: Bitmask> AckBlock<B> {
	/// Create an instance with the mask completely set.
	/// This is useful for tracking outgoing packets, as the buffer can advance up to 32 steps
	/// before yielding a [`HoleError`].
	pub fn preset() -> Self {
		Self {
			index: SeqId::zero(),
			mask: B::filled(),
		}
	}

	/// Create an instance with the mask free.
	/// This is useful for tracking incoming packets, although it does erroneously consider the
	/// 0-index sequence id to have been received.
	pub fn empty() -> Self {
		Self {
			index: SeqId::zero(),
			mask: B::empty(),
		}
	}

	/// Update `self` to include as many set bits from supplied `other` as possible. The implementation
	/// assumes that all packets preceding the ids within `self` have been delivered already.
	pub fn update(&mut self, other: &Self) -> AckSequence<B> {
		if other.index >= self.index {
			// the index should only ever increase, so it's safe to replace self.mask with other
			let result = AckSequence::new(other, self);
			self.index = other.index;
			self.mask = other.mask;
			result
		} else {
			AckSequence(None)
		}
	}

	/// Get the maximum index that could be used in [`set_strict`](Self::set_strict) without causing
	/// a [`HoleError`].
	#[inline]
	pub fn max_safe_set_idx(&self) -> SeqId {
		self.index + self.mask.highest_ones() as u16
	}

	/// Is the provided index currently set?
	pub fn is_set(&self, index: SeqId) -> Result<bool, OutOfBoundsError> {
		if index == self.index {
			return Ok(true);
		}
		let dist = self.index - index;
		if dist > B::MAX_BITS {
			return Err(OutOfBoundsError);
		}
		Ok(self.mask.is_set(dist - 1))
	}

	/// Shorthand for [`is_set`](Self::is_set) unwrapping an error into `false` value
	#[inline]
	pub fn is_set_and_in_bounds(&self, index: SeqId) -> bool {
		self.is_set(index).unwrap_or(false)
	}

	/// Set the provided index if such an operation will not make later acknowledgement of any
	/// unacknowledged packets impossible.
	///
	/// If such behavior is not required use [`set_lossy`](Self::set_lossy).
	#[inline]
	pub fn set_strict(&mut self, index: SeqId) -> Result<(), HoleError> {
		let hole_would_form = self.set_internal(index, false);
		if hole_would_form {
			Err(HoleError)
		} else {
			Ok(())
		}
	}

	/// Set the provided index, possibly making some preceding unacknowledged [`SeqId`] not
	/// acknowledgeable using this [`AckBlock`].
	///
	/// Returns whether a some preceding [`SeqId`] is no longer acknowledgeable using this [`AckBlock`].
	#[inline]
	pub fn set_lossy(&mut self, index: SeqId) -> bool {
		self.set_internal(index, true)
	}

	/// Set the provided index.
	///
	/// Returns whether a some preceding [`SeqId`] is no longer acknowledgeable using this [`AckBlock`].
	///
	/// # Lossiness
	/// If `allow_loss` is `true` - the index will always be set, updating self.
	///
	/// If `allow_loss` is `false` and a hole would be left behind - the operation is aborted early.
	fn set_internal(&mut self, index: SeqId, allow_loss: bool) -> bool {
		if index == self.index {
			return false;
		}

		let reverse_dist = self.index - index;
		if reverse_dist <= B::MAX_BITS {
			self.mask.set(reverse_dist - 1);
			return false;
		};

		let dist = index - self.index;
		if dist <= B::MAX_BITS && self.mask.highest_ones() >= dist {
			self.mask.shift(dist);
			// include the previous index
			self.mask.set(dist - 1);
			self.index = index;
			false
		} else {
			if allow_loss {
				self.mask = B::empty();
				self.index = index;
			}
			true
		}
	}
}

impl<B: Bitmask + fmt::Binary> fmt::Display for AckBlock<B> {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{{index: {}; mask: {:b}}}", &self.index.0, &self.mask)
	}
}

/// An iterable freshly acknowledged ack sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AckSequence<B: Bitmask>(Option<(SeqId, B)>);

impl<B: Bitmask> AckSequence<B> {
	/// Construct a new ack sequence, assuming that the new index has not been received before.
	fn new(new: &AckBlock<B>, old: &AckBlock<B>) -> Self {
		let gap = (new.index - old.index).min(B::MAX_BITS);
		if gap == 0 {
			return Self::from_matching(new, old);
		}

		let mut mask = new.mask;
		let mut old = old.mask;
		old.shift(1);
		old.set(0);
		old.shift(gap - 1);

		mask.unset_every(old);
		Self(Some((new.index, mask)))
	}

	fn from_matching(new: &AckBlock<B>, old: &AckBlock<B>) -> Self {
		let mut mask = new.mask;
		mask.unset_every(old.mask);
		if mask.is_empty() {
			return Self(None);
		}
		let gap = mask.lowest_zeros();
		let index = SeqId(new.index.0 - gap);
		mask.unshift(gap);
		Self(Some((index, mask)))
	}

	/// Number of remaining acknowledged ids.
	#[inline]
	pub fn count(self) -> u16 {
		match self.0 {
			Some((_, mask)) => mask.count_ones() + 1,
			None => 0,
		}
	}
}

impl<B: Bitmask> From<AckBlock<B>> for AckSequence<B> {
	fn from(value: AckBlock<B>) -> Self {
		if value.mask.is_empty() && value.index == SeqId::zero() {
			return Self(None);
		}
		Self(Some((value.index, value.mask)))
	}
}

impl<B: Bitmask> Iterator for AckSequence<B> {
	type Item = SeqId;

	fn next(&mut self) -> Option<Self::Item> {
		let (last, mask) = self.0.as_mut()?;
		if mask.is_empty() {
			let last = *last;
			self.0 = None;
			Some(last)
		} else {
			let result = SeqId(
				last.0
					.wrapping_sub(B::MAX_BITS)
					.wrapping_add(mask.lowest_zeros()),
			);
			mask.unset(mask.lowest_zeros());
			Some(result)
		}
	}

	#[inline]
	fn size_hint(&self) -> (usize, Option<usize>) {
		let count = self.count() as usize;
		(count, Some(count))
	}
}

#[cfg(test)]
mod test {
	use super::*;

	fn seq(index: u16) -> SeqId {
		SeqId(index)
	}

	#[test]
	fn seq_id_buffer_detects_set_bit() {
		let mut buffer = AckBlock::<u32>::preset();
		buffer.set_strict(seq(1)).unwrap();

		assert!(buffer.is_set(seq(1)).unwrap());

		assert_eq!(buffer.is_set(seq(2)), Err(OutOfBoundsError));
		assert!(!buffer.is_set_and_in_bounds(seq(2)));

		buffer.set_strict(seq(2)).unwrap();
		assert!(buffer.is_set(seq(1)).unwrap());
		assert!(buffer.is_set(seq(2)).unwrap());

		assert_eq!(buffer.set_strict(seq(256)), Err(HoleError));
		buffer.set_lossy(seq(256));
		assert!(buffer.is_set_and_in_bounds(seq(256)));
		assert!(!buffer.is_set_and_in_bounds(seq(255)));
	}

	#[test]
	fn seq_id_buffer_max_set_bit() {
		let mut buffer = AckBlock::<u32>::preset();
		let max_idx = buffer.max_safe_set_idx();
		buffer.set_strict(max_idx).unwrap();

		let mut buffer = AckBlock::<u32>::preset();
		let max_idx = buffer.max_safe_set_idx() + 1;
		assert_eq!(buffer.set_strict(max_idx), Err(HoleError));
	}

	#[test]
	fn ack_seq_following_packets() {
		let old = AckBlock {
			index: seq(1),
			mask: 0u32,
		};
		let new = AckBlock {
			index: seq(2),
			mask: 0u32,
		};

		let acks: Vec<SeqId> = AckSequence::new(&new, &old).collect();
		assert_eq!(acks, vec![seq(2)]);
	}

	#[test]
	fn ack_seq_gap() {
		let old = AckBlock {
			index: seq(1),
			mask: 0u32,
		};
		let new = AckBlock {
			index: seq(3),
			mask: 0u32,
		};
		let acks: Vec<SeqId> = AckSequence::new(&new, &old).collect();
		assert_eq!(acks, vec![seq(3)]);
	}

	#[test]
	fn ack_seq_gap_with_mask() {
		let old = AckBlock {
			index: seq(1),
			mask: 0u32,
		};
		let new = AckBlock {
			index: seq(4),
			mask: 0b11u32 << (u32::BITS - 2),
		};

		let acks: Vec<SeqId> = AckSequence::new(&new, &old).collect();
		assert_eq!(acks, vec![seq(2), seq(3), seq(4)]);
	}
}
