use std::time::{Duration, Instant};

/// Trait for determining when to re-send a packet.
pub trait ResendStrategy {
	/// Metadata for determining when to resend a given packet.
	///
	/// This data is typically NOT sent with the packet itself.
	type Meta;

	/// Get metadata for a packet that is about to be sent.
	///
	/// Will only be called for new (packets) not ones about to be re-sent.
	#[must_use]
	fn build_metadata(&mut self) -> Self::Meta;

	/// Determine if a packet was lost, given its associated metadata and the number of received
	/// acks of later packets (ones sent after the one in question).
	///
	/// Should return whether a packet should be re-sent.
	#[must_use]
	fn is_lost(&mut self, meta: &mut Self::Meta, later_acks: u16) -> bool;
}

/// Treat a packet as lost if the number of known delivered packets sent after the one in question
/// exceeds some threshold.
///
/// Note: this strategy can cause a lot of unnecessary re-sends if the
/// [`packets_to_resend`](super::LossDetector::packets_to_resend) is called multiple times within a
/// single round-trip-time. Consider using [`LostIfRttPassed`] instead.
#[derive(Debug, Clone, Copy)]
pub struct ResendIfMoreLaterAcks(u16);

impl ResendIfMoreLaterAcks {
	#[inline]
	pub const fn new(later_acks: u16) -> Self {
		Self(later_acks)
	}
}

impl ResendStrategy for ResendIfMoreLaterAcks {
	type Meta = ();

	#[inline]
	fn build_metadata(&mut self) -> Self::Meta {
		()
	}

	#[inline]
	fn is_lost(&mut self, _: &mut Self::Meta, later_acks: u16) -> bool {
		later_acks > self.0
	}
}

/// Treat a packet as loft if we received more than the provided number of later acks.
#[derive(Debug, Clone, Copy)]
pub struct ResendIfRttPassed {
	estimated_rtt: Duration,
	rtt_factor: f32,
}

impl ResendIfRttPassed {
	#[inline]
	pub fn new(rtt_factor: f32) -> Self {
		Self {
			estimated_rtt: Duration::from_secs(1),
			rtt_factor,
		}
	}

	/// Update the estimated round-trip-time used by this strategy.
	pub fn update_rtt(&mut self, rtt: Duration) {
		self.estimated_rtt = rtt;
	}

	#[inline]
	fn resend_duration(&self) -> Duration {
		self.estimated_rtt.mul_f32(self.rtt_factor)
	}
}

impl ResendStrategy for ResendIfRttPassed {
	type Meta = Instant;

	#[inline]
	fn build_metadata(&mut self) -> Self::Meta {
		Instant::now()
	}

	fn is_lost(&mut self, meta: &mut Self::Meta, _: u16) -> bool {
		let now = Instant::now();
		if now - *meta >= self.resend_duration() {
			*meta = now;
			true
		} else {
			false
		}
	}
}

/// Helper initialized for netkit supplied resend strategies
pub struct ResendIf;

impl ResendIf {
	/// Resend any packet if more than `n` packets sent after the one in question have been delivered
	#[inline]
	pub fn later_acks_gt(n: u16) -> ResendIfMoreLaterAcks {
		ResendIfMoreLaterAcks(n)
	}

	/// Resend any packet if at least `n` packets sent after the one in question have been delivered
	pub fn later_acks_ge(n: u16) -> ResendIfMoreLaterAcks {
		ResendIfMoreLaterAcks(n + 1)
	}

	/// Resend a packet if it was not delivered within [`estimated_rtt`](ResendIfRttPassed::update_rtt)
	/// `* factor` since the last time it was sent (or re-sent).
	#[inline]
	pub fn no_ack_for_rtt(factor: f32) -> ResendIfRttPassed {
		ResendIfRttPassed::new(factor)
	}
}
