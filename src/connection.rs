//! Application-based connection management.
//!
//! The idea of a connection is a persistent state machine across back-and-forth network traffic
//! between two different nodes. This can be useful even on top of connection-based protocols like
//! TCP, by providing means for application-level initialization logic.
//!
//! # Examples
//! Establishing a connection from a client:
//! ```
//! use netforge::Connection;
//!
//! let connection = Connection::request(...).unwrap();
//! let socket = UdpSocket::bind("127.0.0.1").unwrap();
//! socket.connect(connection.peer_addr()).unwrap();
//!
//! let mut buffer = [0u8; 1200];
//! connection.header().serialize_into(&mut buffer);
//! ```
//!
//! # Notes
//! Only one pending connection is assumed per peer (socket-addr). Attempting to request multiple
//! connections from the same socket will result in undefined behavior!

use crate::serializable;
use std::{
	io,
	net::SocketAddr,
	ops::{BitAnd, Not},
};
use thiserror::Error;

#[cfg(feature = "client")]
use std::net::ToSocketAddrs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
	/// A new connection is pending.
	///
	/// # Client side
	/// A client is waiting on the server to accept the connection.
	///
	/// # Server side
	/// A new client is attempting to connect and is waiting to be accepted (or rejected).
	Pending,
	/// A connection has been accepted and is fully functional.
	Accepted,
	/// A connection has been discontinued or not accepted within expected time slot.
	Dropped,
}

mod private {
	pub trait Sealed {}
	impl Sealed for u8 {}
	impl Sealed for u16 {}
	impl Sealed for u32 {}
}

#[derive(Debug, Error)]
#[error("Ran out of available connection ids!")]
pub struct OutOfIdsError;

/// The backing for a [`ConnectionId`], allows use of either [`u8`], [`u16`] or [`u32`] for
/// identifying a connection. Using a larger integer allows for more simultaneously active
/// connections, but imposes larger serialized size.
pub trait ConnectionWord:
	private::Sealed + Copy + Eq + Not<Output = Self> + BitAnd<Output = Self>
{
	const TOP_BIT: Self;
	const ZERO: Self;
	const MAX_COUNT: usize;
	fn next(self) -> Result<Self, OutOfIdsError>;
}

macro_rules! impl_word {
	($($t:ty),+ $(,)?) => {
		$(
			impl ConnectionWord for $t {
				const TOP_BIT: Self = 1 << (<$t>::BITS - 1);
				const ZERO: Self = 0;
				const MAX_COUNT: usize = (<$t>::MAX & !Self::TOP_BIT) as usize;
				#[inline]
				fn next(self) -> Result<Self, OutOfIdsError> {
					let result = self + 1;
					if result & Self::TOP_BIT == 0 {
						Ok(result)
					} else {
						Err(OutOfIdsError)
					}
				}
			}
		)+
	};
}

impl_word! { u8, u16, u32 }

/// Unique identifier for a connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConnectionId<W: ConnectionWord>(W);

impl<W: ConnectionWord> ConnectionId<W> {
	/// Maximum concurrent active connections possible using [this identifier](Self).
	pub const MAX_COUNT: usize = W::MAX_COUNT;

	#[inline]
	#[must_use]
	fn new(id: W) -> Option<Self> {
		(id != W::ZERO).then_some(Self(id))
	}

	#[inline]
	#[must_use]
	const fn placeholder() -> Self {
		Self(W::ZERO)
	}

	/// Get the next connection id.
	#[inline]
	fn inc(&mut self) -> Result<(), OutOfIdsError> {
		self.0 = self.0.next()?;
		Ok(())
	}

	#[inline]
	fn as_option(self) -> Option<Self> {
		(self.0 != W::ZERO).then_some(Self(self.0))
	}
}

/// Header section used for managing connections.
///
/// Under the hood combines a command flag as well as connection id.
#[serializable]
#[derive(Debug, Clone, Copy)]
pub struct ConnectionSection<W: ConnectionWord>(W);

impl<W: ConnectionWord> ConnectionSection<W> {
	#[inline]
	pub fn connection_id(self) -> Option<ConnectionId<W>> {
		ConnectionId::new(self.0 & !W::TOP_BIT)
	}

	#[cfg(feature = "client")]
	/// Construct [`ConnectionSection`] requesting a new connection.
	#[inline]
	pub fn request() -> Self {
		Self(W::ZERO)
	}

	#[cfg(feature = "server")]
	/// Construct [`ConnectionSection`] accepting a new connection and giving it provided id.
	#[inline]
	pub fn accept(id: ConnectionId<W>) -> Self {
		Self(id.0)
	}

	#[inline]
	pub fn drop() -> Self {
		Self(W::TOP_BIT)
	}

	#[inline]
	fn next_state(self) -> ConnectionState {
		match self.0 {
			i if i == W::ZERO => ConnectionState::Pending,
			i if i & W::TOP_BIT == W::TOP_BIT => ConnectionState::Dropped,
			_ => ConnectionState::Accepted,
		}
	}
}

impl<W: ConnectionWord> From<ConnectionId<W>> for ConnectionSection<W> {
	#[inline]
	fn from(value: ConnectionId<W>) -> Self {
		Self(value.0)
	}
}

/// An idea of a connection between 2 separate nodes.
#[derive(Debug, Clone)]
pub struct Connection<W: ConnectionWord> {
	id: ConnectionId<W>,
	state: ConnectionState,
	peer_addr: SocketAddr,
}

impl<W: ConnectionWord> Connection<W> {
	#[inline]
	#[must_use]
	pub fn id(&self) -> Option<ConnectionId<W>> {
		self.id.as_option()
	}

	#[inline]
	#[must_use]
	pub fn state(&self) -> ConnectionState {
		self.state
	}

	/// Get the associated peer (other end of the connection) address.
	#[inline]
	#[must_use]
	pub fn peer_addr(&self) -> SocketAddr {
		self.peer_addr
	}

	/// Get the header section that should be sent with any packets for maintaining the connection.
	#[inline]
	#[must_use]
	pub fn header_section(&self) -> Option<ConnectionSection<W>> {
		match self.state {
			ConnectionState::Pending => Some(ConnectionSection::request()),
			ConnectionState::Accepted => Some(self.id.into()),
			ConnectionState::Dropped => None,
		}
	}

	/// Attempt to establish a branch new connection to the provided address.
	#[cfg(feature = "client")]
	pub fn request<A: ToSocketAddrs>(peer_addr: A) -> Result<Self, io::Error> {
		let addr = peer_addr.to_socket_addrs()?.next().ok_or(io::Error::new(
			io::ErrorKind::InvalidInput,
			"could not resolve to any addresses",
		))?;
		Ok(Self {
			id: ConnectionId::placeholder(),
			state: ConnectionState::Pending,
			peer_addr: addr,
		})
	}

	#[cfg(feature = "server")]
	#[inline]
	#[must_use]
	pub fn accept(peer_addr: SocketAddr, id: ConnectionId<W>) -> Self {
		Self {
			id,
			state: ConnectionState::Accepted,
			peer_addr,
		}
	}
}

/// Allocator for connection ids, facilitates reuse of no-longer active connection ids.
pub struct ConnectionIdAllocator<W: ConnectionWord> {
	next: ConnectionId<W>,
	free: Vec<ConnectionId<W>>,
}

impl<W: ConnectionWord> ConnectionIdAllocator<W> {
	#[inline]
	pub fn new() -> Self {
		Self {
			next: ConnectionId::placeholder(),
			free: Vec::new(),
		}
	}

	pub fn alloc(&mut self) -> Result<ConnectionId<W>, OutOfIdsError> {
		if let Some(id) = self.free.pop() {
			return Ok(id);
		}
		self.next.inc()?;
		Ok(self.next)
	}

	#[inline]
	pub fn free(&mut self, id: ConnectionId<W>) {
		self.free.push(id);
	}
}
