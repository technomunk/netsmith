//! Reliability layer over an unreliable channel (like UDP).
//!
//! Based on ideas from [Glenn Fisher](https://gafferongames.com/post/reliability_ordering_and_congestion_avoidance_over_udp/)

// TODO: expand documentation

use crate::serializable;
use bitmask::Bitmask;
use seq::{AckBlock, SeqId};

pub mod bitmask;
pub mod detect;
pub mod resend;
pub mod seq;
pub mod slide;
pub mod strategy;

pub use detect::DeliveryDetector;
pub use resend::ResendCoordinator;
pub use strategy::ResendIf;

/// Header section used for packet delivery detection and/or reliable delivery.
#[serializable]
#[derive(Debug, Clone)]
pub struct AckSection<B: Bitmask = u32> {
	index: SeqId,
	acks: AckBlock<B>,
}

impl<B: Bitmask> Default for AckSection<B> {
	fn default() -> Self {
		Self {
			index: Default::default(),
			acks: AckBlock::empty(),
		}
	}
}
