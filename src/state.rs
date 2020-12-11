//! The State layer.
//!
//! The state is responsible for holding the data and for exposing a way to get and set values to
//! and from it.
//!
//! The [State] trait does its best to assume nothing about the data an implementor might hold. It
//! can be a Key/Value store like the [default State] implementation, a blob of binary data or anything else an implementor wishes for.
//!
//! It does so by using a [StateValue] trait with default implementations of serializing the data to
//! different formats. For example, the default state implementation implements a serialization of
//! StateValue to JSON.
//!
//! The state layer can choose to implement other mechanism related to the state, like TTL,
//! data compression, etc.
//!
//! [State]: State
//! [default State]: default
//! [StateValue]: StateValue

pub mod default;
pub mod data_seeder;

use std::error::Error as StdError;
use std::sync::Arc;

/// An atomic reference to a state.
///
/// This type makes it so that we can share the state across threads.
pub type SafeState = Arc<dyn State>;

/// A trait used as a protocol between different components that use the state.
///
/// Both the Connection and Agent layers use the state to get and set values. They do not know
/// anything about the implementation of one another. This trait allows them to use the same state
/// without assuming anything about the structure of a state value.
///
/// The trait offers functions to serialize to different formats. The default connection layer for
/// example, serializes a state value to json to be exchanged with other peers.
///
/// The functions in this trait have a default implementation that always return `None` so that
/// implementors don't have to bother and implement all serialization functions that are irrelevant
/// to them. If an implementor of an Agent layer, for example, tries to serialize a state value that the State layer does not
/// support, it will get a `None` value in response. This might indicate an incompatible usage of
/// an Agent and State layers.
///
/// When a user of the c19 protocol chooses their state, agent and connection layers, they should
/// ensure that each component is compatible with one another.
///
/// An implementor of layer might choose to run different compatability tests at startup and notify
/// the user of incompatabilities.
pub trait StateValue: Send + Sync {
    fn as_bytes(&self) -> Option<Vec<u8>>;
}

/// The State trait.
///
/// Every state implementor must implement this trait. It is first auto-loaded by the configuration
/// deserializer and then initialized by the library by calling the `init` function. The `init`
/// function should return a [SafeState] which is then passed to the connection and agent layers.
#[typetag::serde(tag = "kind")]
pub trait State: std::fmt::Debug + Send + Sync + CloneState {
    /// Initializes the state and returns a state that is safe to be shared across threads.
    ///
    /// A state object is already loaded by the configuration. The implementor can use this
    /// function to add or initialize any other relevant data and then return a SafeState object
    /// which is shared with the connection and agent layers.
    fn init(&self) -> SafeState;

    /// Returns the version of the current state.
    ///
    /// An implementor can use this function to keep a version for each "state" of the sate. For
    /// example, the default state implementation sets this value to 
    /// a random string whenever the state changes. It is then saves that version in a version history which 
    /// allows for the connection layer to compute diff between two versions. 
    fn version(&self) -> String;

    /// Sets a value to the state.
    ///
    /// There's no assumption about the value itself. It can be anything the implementor wishes.
    /// The default state implementation, for example, treats this value as a map of key/value
    /// pairs where the key is a String and the value conforms to a serde_json::Value value.
    fn set(&self, value: &dyn StateValue) -> Result<(), Box<dyn StdError>>;

    /// Gets the value associated with the specified key.
    ///
    /// To allow maximum flexibility, the key itself is a StateValue, which in effect means it can
    /// be anything desired by the implementor.
    fn get(&self, key: &dyn StateValue) -> Option<Box<dyn StateValue>>;

    /// Returns the value associated with the specified key or the default if the key was not found 
    /// in the state.
    fn get_or(&self, key: &dyn StateValue, default: Box<dyn StateValue>) -> Box<dyn StateValue> {
        self.get(key).unwrap_or(default)
    }

    /// Returns the difference between this and the `other` state.
    fn diff(&self, other: &dyn StateValue) -> Result<Box<dyn StateValue>, Box<dyn StdError>>;

    /// Returns the whole state as a StateValue.
    ///
    /// This is helpful when the connection layer wishes to publish the whole state to its peers.
    fn get_root(&self) -> Option<Box<dyn StateValue>>;
}

pub trait CloneState {
    fn clone_state(&self) -> Box<dyn State>;
}

impl<T> CloneState for T
where
    T: State + Clone + 'static,
{
    fn clone_state(&self) -> Box<dyn State> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn State> {
    fn clone(&self) -> Self {
        self.clone_state()
    }
}

impl StateValue for &'static str {
    fn as_bytes(&self) -> Option<Vec<u8>> {
        Some(Vec::from(*self))
    }
}

impl StateValue for String {
    fn as_bytes(&self) -> Option<Vec<u8>> {
        Some(Vec::from(self.as_str()))
    }
}

impl<T> From<T> for Box<dyn StateValue> 
where T: StateValue + 'static
{
    fn from(t: T) -> Self {
        Box::new(t)
    }
}
