//! Peer provider.
//!
//! The peer provider is responsible for retreiving peers that are available for exchanging states.
//!
//! The peer provider is specific to the default connection implementation, although it can serve
//! other implemntations in the future. It is implemented as a trait to allow the default
//! connection implementation to use different kind of providers, not only the [K8s](crate::connection::peer_provider::k8s), which
//! is the default one.
//!
//! The provider is first loaded by the configuration and then initialized by the default
//! connection itself. It is then used by the default connection to get the list of available peers
//! to choose from.

pub mod k8s;
pub mod static_peer_provider;
use std::error::Error as StdError;
use std::net::{Ipv4Addr, SocketAddrV4};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Peer {
    Ipv4Addr(Ipv4Addr),
    SocketAddrV4(SocketAddrV4),
}

#[typetag::serde(tag = "kind")]
pub trait PeerProvider: std::fmt::Debug + Send + Sync {
    /// Initializes the peer provider.
    fn init(&self) -> Result<(), Box<dyn StdError + Send + Sync>>;

    /// Returns a vector of available peers.
    fn get(&self) -> Vec<Peer>;
}

impl FromStr for Peer {
    type Err = std::net::AddrParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        SocketAddrV4::from_str(s).and_then(|socket| Ok(Peer::SocketAddrV4(socket))).or_else(|_| {
            Ipv4Addr::from_str(s).and_then(|ip| Ok(Peer::Ipv4Addr(ip)))
        })
    }
}

impl Peer {
    pub fn ip(&self) -> &Ipv4Addr {
        match self {
            Peer::Ipv4Addr(s) => s,
            Peer::SocketAddrV4(s) => s.ip(),
        }
    }

    pub fn port(&self) -> Option<u16> {
        match self {
            Peer::Ipv4Addr(_) => None,
            Peer::SocketAddrV4(s) => Some(s.port()),
        }
    }
}
