//! The Connection layer.
//!
//! The connection layer is responsible for exchaging the state with other peers.
//!
//! There can be many implementations for the connection layer. One could choose the protocol, the
//! transport layer, how to choose peers to exchange the state with, the rate in which the state is
//! being exchanged, compression, etc...
//!
//! See the [default connection] layer implementation for some ideas.
//!
//! [default connection]: default

pub mod default;
pub mod peer_provider;

use crate::state;
use futures::future::BoxFuture;
use std::error::Error as StdError;

/// The Connection Trait.
///
/// The only required method is the `start` method where the state is passed to the implementation
/// to should be used to set and get values to and from the state.
///
/// See the default connection implementation for an idea on how to implement a connection layer.
#[typetag::serde(tag = "kind")]
pub trait Connection: std::fmt::Debug + Send + Sync {
    fn start<'a>(
        &'a self,
        state: state::SafeState,
    ) -> BoxFuture<'a, Result<(), Box<dyn StdError + Send + Sync>>>;
}
