//! Building blocks for bespoke networking protocols. Each block (component) is responsible for doing
//! only one thing, while imposing minimal runtime overhead and architecture constraints.
//!
//! # Features
//! - [reliable delivery](reliability)
//! - [connection management](connection)

#[cfg(any(feature = "client", feature = "server"))]
pub mod connection;
#[cfg(feature = "reliability")]
pub mod reliability;


#[cfg(any(feature = "client", feature = "server"))]
pub use connection::Connection;
#[cfg(feature = "reliability")]
pub use reliability::{DeliveryDetector, ResendCoordinator, ResendIf};

pub(crate) use netforge_macros::serializable;
