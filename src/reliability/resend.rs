use super::{
	AckSection, DeliveryDetector,
	bitmask::Bitmask,
	seq::{AckSequence, HoleError, SeqId},
	slide::TrackingSlide,
	strategy::ResendStrategy,
};
use core::fmt;
use thiserror::Error;

#[derive(Debug, Error)]
#[error(
	"maximum number of packets that can be in flight was reached, no further packets can be sent until some are received by the remote connection"
)]
pub struct MaxPacketsInFlightError;

impl From<HoleError> for MaxPacketsInFlightError {
	#[inline]
	fn from(_: HoleError) -> Self {
		Self
	}
}

/// Provide reliability over an unreliable delivery channel by detecting possibly lost packets and
/// re-sending them.
///
/// Note that actual re-sending should be done by the calling code, as well as ensuring that the
/// re-sent frame data is identical between sent and re-sent packets, as otherwise some data may
/// never be delivered.
///
/// This implementation provides simple means of ensuring reliable delivery with smaller overhead,
/// however it may be inefficiently using the delivery channel, if the packets are tiny.
pub struct ResendCoordinator<S: ResendStrategy, B: Bitmask = u32, const N: usize = 32> {
	dd: DeliveryDetector<B>,
	strat: S,
	meta: TrackingSlide<S::Meta, SeqId, N>,
}

impl<S: ResendStrategy, B: Bitmask, const N: usize> ResendCoordinator<S, B, N> {
	#[inline]
	#[must_use]
	pub fn new(strat: S) -> Self {
		debug_assert!(N == B::MAX_BITS as usize, "N must equal B::MAX_BITS!");

		Self {
			dd: DeliveryDetector::new(),
			strat,
			meta: TrackingSlide::default(),
		}
	}

	/// Prepare a (new) packet to be sent.
	/// The packet is considered "in flight" after this function call.
	#[must_use]
	pub fn prepare_send(&mut self) -> Result<AckSection<B>, MaxPacketsInFlightError> {
		let result = self.dd.prepare_send_strict()?;
		let meta = self.strat.build_metadata();
		self.meta.push(meta);
		Ok(result)
	}

	/// Prepare a previously-sent packet to be sent again, updating its ack-header. Note that it's
	/// the responsibility of the caller to ensure that the resent packet contains the same frame.
	#[must_use]
	#[inline]
	pub fn prepare_resend(&mut self, index: SeqId) -> AckSection<B> {
		self.dd.prepare_resend(index)
	}

	/// Get an iterable over packet ids that should be resend according the the [strategy](DetermineLoss).
	///
	/// Note that the strategy may assume the packet is re-sent right away, so the exact values
	/// yielded by the iterator may vary from call to call.
	#[must_use]
	pub fn packets_to_resend<'a>(&'a mut self) -> impl 'a + Iterator<Item = SeqId> {
		let mut later_acks = self.dd.delivered_packets().count();
		let dd = &self.dd;
		let strat = &mut self.strat;
		self.meta.iter_mut().filter_map(move |(index, meta)| {
			if dd.is_delivered(index) {
				later_acks -= 1;
				return None;
			}

			if strat.is_lost(meta, later_acks) {
				Some(index)
			} else {
				None
			}
		})
	}

	/// Record received packets. Returns an iterable sequence of the newly received packets.
	#[inline]
	pub fn record_received(&mut self, header: &AckSection<B>) -> Result<AckSequence<B>, HoleError> {
		self.dd.record_received_strict(header)
	}

	/// Get reference to the used [`ResendStrat`].
	#[inline]
	pub fn strategy(&self) -> &S {
		&self.strat
	}

	/// Get a mutable reference to the used [`ResendStrat`].
	#[inline]
	pub fn strategy_mut(&mut self) -> &mut S {
		&mut self.strat
	}

	/// Replace the used [`ResendStrat`] with a new one.
	///
	/// Note that this is a relatively cheap operation (as no new allocations are performed), which
	/// requires the new strategy to use the same metadata as the old one. If a completely different
	/// strategy is required - use [`with_new_strategy`](Self::with_new_strategy).
	#[inline]
	pub fn with_strategy<NS>(self, strategy: NS) -> ResendCoordinator<NS, B, N>
	where
		NS: ResendStrategy<Meta = S::Meta>,
	{
		ResendCoordinator {
			dd: self.dd,
			strat: strategy,
			meta: self.meta,
		}
	}

	/// Replace the used [`ResendStrat`] with a new one.
	///
	/// # Notes
	/// - This operation requires remapping internal packet buffer to accommodate the new strategy metadata. If the new strategy uses the same metadata - use [`with_strategy`](Self::with_strategy).
	/// - The strategy will build metadata for all cached packets, possibly doing redundant work.
	pub fn with_new_strategy<NS>(self, mut strategy: NS) -> ResendCoordinator<NS, B, N>
	where
		NS: ResendStrategy,
	{
		let meta = self.meta.map_elems(|_| strategy.build_metadata());
		ResendCoordinator {
			dd: self.dd,
			strat: strategy,
			meta,
		}
	}
}

impl<S, B: Bitmask, const N: usize> fmt::Debug for ResendCoordinator<S, B, N>
where
	S: ResendStrategy + fmt::Debug,
	S::Meta: fmt::Debug,
	B: fmt::Debug,
{
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ReliableDelivery")
			.field("dd", &self.dd)
			.field("strat", &self.strat)
			.field("meta", &self.meta)
			.finish()
	}
}

impl<S, B: Bitmask, const N: usize> Clone for ResendCoordinator<S, B, N>
where
	S: ResendStrategy + Clone,
	TrackingSlide<S::Meta, SeqId, N>: Clone,
{
	fn clone(&self) -> Self {
		Self {
			dd: self.dd.clone(),
			strat: self.strat.clone(),
			meta: self.meta.clone(),
		}
	}
}
