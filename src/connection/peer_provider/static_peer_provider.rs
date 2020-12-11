//! An implementation of the peer provider.
//!
//! This implementation uses a static list of peers specified directly in the 
//! configuration file.
//!
//! It's helpful when developing and running the agent locally.
//!
//! # Example:
//!
//! ```yaml
//!     peer_provider:
//!       kind: Static
//!       peers:
//!         - 127.0.0.1
//! ```
//!
use crate::connection::peer_provider::{PeerProvider, Peer};
use serde::{Deserialize, Serialize};
use std::error::Error as StdError;

type Result<T> = std::result::Result<T, Box<dyn StdError + Send + Sync>>;

#[derive(Serialize, Deserialize, Debug)]
pub struct Static {
    peers: Vec<Peer>,
}

#[typetag::serde]
impl PeerProvider for Static {
    fn init(&self) -> Result<()> {
        Ok(())
    }

    fn get(&self) -> Vec<Peer> {
        self.peers.clone()
    }
}
