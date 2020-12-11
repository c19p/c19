//! The Agent layer.
//!
//! The Agent is responsible for communicating with the application layer.
//! It exposes endpoints to allow an application to get and set data from and to the state.
//!
//! The Agent trait assumes nothing about the state and the agent implementation. Agent implementors
//! should consider how they wish to expose the agent to the app. Which protocol to use (HTTP for
//! example) and what endpoints to expose (get and set for example).
//!
//! The [default agent] implementation exposes an HTTP GET and PUT endpoints to allow an app to get
//! and set key/value pairs to the state. See the [default agent] implementation documentation below for more details.
//!
//! [default agent]: crate::agent::default

pub mod default;

use crate::state;
use futures::future::BoxFuture;
use std::error::Error as StdError;

/// The Agent trait.
///
/// The only required method is start.
/// The start method accepts a reference to the current state. Agent implementors should hold on
/// that reference for their use. The run method will make sure to
/// start the agent with an initialized state.
#[typetag::serde(tag = "kind")]
pub trait Agent: std::fmt::Debug + Send + Sync {
    fn start<'a>(
        &'a self,
        state: state::SafeState,
    ) -> BoxFuture<'a, Result<(), Box<dyn StdError + Send + Sync>>>;
}
