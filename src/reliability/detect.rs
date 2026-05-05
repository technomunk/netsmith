use super::{
	bitmask::Bitmask,
	seq::{AckBlock, AckSequence, HoleError, SeqId},
	AckSection,
};

/// Since UDP doesn't guarantee that sent packets will be received on the other side - it can be
/// useful to detect that sent packets have been delivered. This struct provides such functionality.
///
/// # Example
/// ```
/// let mut dd = DeliveryDetector::new();
/// let received_headers: &[AckHeader] = ...;
///
/// // Process all received packet first
/// for header in &received_headers {
/// 	dd.record_received_lossy(header);
/// }
/// // Prepare a new header to be sent
/// let header_to_send = dd.prepare_send();
/// ```
///
/// # Strict / lossy
/// Delivery detection can be used as in a best-effort (lossy) fashion - possibly yielding false
/// negatives for some packets, but not imposing any restrictions on sending new packets. This mode
/// can be used if not all packets going through delivery detection are expected to be delivered.
///
/// Or in a strict fashion - returning a [`HoleError`] if sending a new packet would make it
/// impossible to inform the remote connection that a sequence id was received. This mode should be
/// used if all packets going through delivery detection are expected to be delivered.
///
/// | mode | send     | receive |
/// |------|----------|---------|
/// |strict|[`prepare_send_strict`](Self::prepare_send_strict)|[`record_received_strict`](Self::record_received_strict)|
/// |lossy |[`prepare_send_lossy`](Self::prepare_send_lossy)|[`record_received_lossy`](Self::record_received_lossy)|
#[derive(Debug, Clone)]
pub struct DeliveryDetector<B: Bitmask = u32> {
	outgoing: AckSection<B>,
	delivered: AckBlock<B>,
}

impl<B: Bitmask> DeliveryDetector<B> {
	pub fn new() -> Self {
		Self {
			outgoing: AckSection {
				index: SeqId::zero(),
				acks: AckBlock::preset(),
			},
			// HACK: consider unsent packets "delivered" to simplify initialization logic
			delivered: AckBlock::preset(),
		}
	}

	/// The [`SeqId`] of the next outgoing packet.
	#[inline]
	#[must_use]
	pub const fn next_index(&self) -> SeqId {
		self.outgoing.index.next()
	}

	/// Check whether the provided [`SeqId`] has been delivered.
	#[inline]
	pub fn is_delivered(&self, index: SeqId) -> bool {
		self.delivered.is_set_and_in_bounds(index)
	}

	/// Prepare an AckHeader to be resent using an already sent [`SeqId`].
	#[inline]
	pub fn prepare_resend(&self, index: SeqId) -> AckSection<B> {
		AckSection {
			index,
			..self.outgoing
		}
	}

	/// Prepare an [`AckHeader`] to be sent.
	///
	/// The resulting header will only include already processed acks, meaning that
	/// [`record_received_strict`](Self::record_received_strict) should be called for all already
	/// received packets.
	///
	/// This is strict implementation. See [struct level documentation](Self) for more details.
	#[must_use]
	pub fn prepare_send_strict(&mut self) -> Result<AckSection<B>, HoleError> {
		if self.delivered.max_safe_set_idx() > self.outgoing.index {
			self.outgoing.index.inc();
			Ok(self.outgoing.clone())
		} else {
			Err(HoleError)
		}
	}

	/// Prepare an [`AckHeader`] to be sent.
	///
	/// The resulting header will only include already processed acks, meaning that
	/// [`record_received_lossy`](Self::record_received_lossy) should be called for all already
	/// received packets.
	///
	/// This is lossy implementation. See [struct level documentation](Self) for more details.
	#[must_use]
	pub fn prepare_send_lossy(&mut self) -> AckSection<B> {
		self.outgoing.index.inc();
		self.outgoing.clone()
	}

	/// Process incoming [`AckHeader`], updating known delivered packets.
	///
	/// This should be called for all already received packets before
	/// [`prepare_send`](Self::prepare_send_strict) is called (and a new packet is sent).
	///
	/// This is the strict implementation. See [struct level documentation](Self) for more details.
	pub fn record_received_strict(
		&mut self,
		header: &AckSection<B>,
	) -> Result<AckSequence<B>, HoleError> {
		self.outgoing.acks.set_strict(header.index)?;
		Ok(self.delivered.update(&header.acks))
	}

	/// Process incoming [`AckHeader`], updating known delivered packets.
	///
	/// This should be called for all already received packets before
	/// [`prepare_send`](Self::prepare_send_lossy) is called (and a new packet is sent).
	///
	/// This is lossy implementation. See [struct level documentation](Self) for more details.
	pub fn record_received_lossy(&mut self, header: &AckSection<B>) -> AckSequence<B> {
		self.outgoing.acks.set_lossy(header.index);
		self.delivered.update(&header.acks)
	}

	/// Get the number of known delivered packets
	#[inline]
	pub fn delivered_packets(&self) -> AckSequence<B> {
		self.delivered.into()
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	type BM = u32;

	#[test]
	fn test_prepare_send_fails_after_max_packets() {
		let mut sender = DeliveryDetector::<BM>::new();

		for _ in 0..BM::MAX_BITS {
			sender.prepare_send_strict().unwrap();
		}

		assert!(sender.prepare_send_strict().is_err());
	}

	#[test]
	fn test_receive_256_packets_in_strict() {
		let mut sender = DeliveryDetector::<BM>::new();
		let mut receiver = DeliveryDetector::<BM>::new();

		for i in 0..256 {
			let header = sender.prepare_send_lossy();
			assert!(
				receiver.record_received_strict(&header).is_ok(),
				"Failed on {i} receive"
			);
		}
	}

	#[test]
	fn test_fail_record_after_max_packets_in_strict() {
		let mut sender = DeliveryDetector::<BM>::new();
		let mut receiver = DeliveryDetector::<BM>::new();

		for _ in 0..BM::MAX_BITS {
			let _ = sender.prepare_send_lossy();
		}

		let header = sender.prepare_send_lossy();
		assert_eq!(receiver.record_received_strict(&header), Err(HoleError));
	}

	#[test]
	fn test_detects_loss() {
		let mut sender = DeliveryDetector::<BM>::new();
		let mut receiver = DeliveryDetector::<BM>::new();

		for i in 0..BM::MAX_BITS {
			let header = sender.prepare_send_lossy();
			if i != 10 {
				receiver.record_received_strict(&header).unwrap();
			}
		}

		let changes = sender
			.record_received_strict(&receiver.prepare_send_strict().unwrap())
			.unwrap();

		assert_eq!(changes.count(), BM::MAX_BITS - 1);

		// DeliveryDetector starts by sending index 1
		let mut index = SeqId::zero().next();
		for i in 0..BM::MAX_BITS {
			assert_eq!(sender.is_delivered(index), i != 10);
			index.inc();
		}
	}
}
