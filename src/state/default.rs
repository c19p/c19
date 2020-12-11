//! The default implementation of the State layer.
//!
//! This implementation is of a Key/Value state where a value can be anything that can be serialized
//! into a JSON representation.
//!
//! The default state has the following behavior:
//! - It's a Key/Value store
//! - It supports TTL
//! - It resolves conflicts using a timestamp
//! - It records version history
//!
//! # Keys
//! This state expects keys to be of a string type.
//!
//! # Values
//! The values are expected to be a JSON of the following form:
//! ```json
//! {"value": <anything JSON>, "ttl": <optional ttl in milliseconds>, "ts": <optional, manual
//! setting of the timestamp of the value>}
//! ```
//!
//! `value`
//!
//! The only required field is the `value` which can be anything JSON. Even a JSON object.
//!
//! `ttl`
//!
//! The `ttl` field is optional. If not used then the default ttl from the state configuration 
//! will be used, if one specified.
//!
//! `ts`
//!
//! The `ts` field is optional and can be used to override the timestamp that is automatically 
//! set for every new value.
//!
//! # TTL
//! The state returns `None` for expired keys. Expired keys are filtered out when the
//! storage is iterated and are purged by a thread running in the background.
//!
//! The purger thread uses the interval settings from the configuration of the state.
//!
//! # Conflicts
//! Since this is a distributed system, the state might be updated by different peers that are not
//! yet in sync. To resolve a conflict where a key is being updated by more than one peer, a
//! timestamp is used. Only a newer key can override an older one. The timestamp should be the
//! timestamp when the key was first created (by the source).
//!
//! # Version History
//! The state records version history for every change that is made to the state.
//! To make sure the version history doesn't get bloated it is being purged on every 
//! `version_ttl` milliseconds.
//!
//! See the [struct@Default] state struct for details on the different fields and configurations. 

use crate::helpers::utils::epoch;
use crate::state::{self, data_seeder::DataSeeder};
use crate::state::StateValue;
use im::hashmap::HashMap;
use serde::{Deserialize, Serialize};
use serde_json;
use std::error::Error as StdError;
use std::sync::{Arc, RwLock};
use tokio::time::{interval_at, Duration, Instant};
use log::{info, warn};
use std::sync::mpsc;
use std::hash::{Hash, Hasher};
use twox_hash::XxHash64;

/// The maximum number of pending async set operations.
///
/// When the channel for async set operations reaches this maximum, 
/// all subsequent set operations will be blocked until pending ops are 
/// dealt with.
const MAX_SET_OPS: usize = 64000;

/// Version information.
///
/// Holds the timestamp where the version was recorded and 
/// the content itself.
#[derive(Debug, Clone)]
struct Version {
    ts: u64,
    storage: HashMap<String, Value>,
}

/// The default state struct.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Default {
    /// The default TTL (in milliseconds) to use if none is specified when setting a new value.
    ttl: Option<u64>,

    /// The interval in milliseconds in which to purge expired values.
    ///
    /// Default value is 1 minute (60000 milliseconds).
    purge_interval: u64,

    /// The [DataSeeder] to use for seeding the data on initialization.
    data_seeder: Option<Arc<RwLock<Box<dyn DataSeeder>>>>,

    /// The version of the current state.
    ///
    /// This is set to a random unique string on every state change.
    #[serde(skip_serializing, skip_deserializing)]
    version: Arc<RwLock<String>>,

    /// The SyncSender channel to use for async set operations
    ///
    /// When a set operation is being commited to the state, the state 
    /// will pass the operation to an async handler which will then commit the 
    /// changes to the state.
    #[serde(skip_serializing, skip_deserializing)]
    tx: Option<mpsc::SyncSender<Vec<u8>>>,

    /// The data storage in the form of a Key/Value hashmap.
    #[serde(skip_serializing, skip_deserializing)]
    storage: Arc<RwLock<HashMap<String, Box<Value>>>>,

    /// Calculating the version is a bit expensive so we use 
    /// the dirty flag to lazily calculate the verison on-demand.
    #[serde(skip_serializing, skip_deserializing)]
    is_dirty: Arc<RwLock<bool>>,
}

impl Default {
    /// Merges the two maps while resolving conflicts.
    ///
    /// A value from the other map will be commited to the state only 
    /// if it has a newer timestamp.
    ///
    /// If there was a change to the sate, the version will be recorded 
    /// in the version history.
    fn set(&self, map: &HashMap<String, Box<Value>>) {
        let map = map.clone();
        let mut is_dirty = false;

        // merge the maps
        let mut storage = self.storage.write().unwrap();
        for (key, mut right) in map {
            if right.is_expired() {
                continue;
            }

            if self.ttl.is_some() && right.ttl.is_none() {
                right.ttl = self.ttl;
            }

            storage.entry(key)
                .and_modify(|v| {
                    if v.ts < right.ts {
                        *v = right.clone().into();
                        is_dirty = true;
                    }})
            .or_insert({
                is_dirty = true;
                right.into()
            }); 
        }

        *self.is_dirty.write().unwrap() = is_dirty;
    }

    /// Purges expired keys.
    fn purge(&self) {
        self.storage.write().unwrap().retain(|_, v| !v.is_expired());
    }

    /// Seeds the state with the data from the DataSeeder.
    fn seed(&self, data_seeder: Arc<RwLock<Box<dyn DataSeeder>>>) -> Result<(), Box<dyn StdError>> {
        data_seeder.read().unwrap().load().and_then(|data| {
            let data: Result<HashMap<String, Box<Value>>, Box<dyn StdError>> = (&*data).into();
            data.and_then(|data| {
                self.set(&data);
                Ok(())
            })
        })
    }
}

/// Default values for the state struct.
impl std::default::Default for Default {
    fn default() -> Self {
        Default {
            ttl: None,
            purge_interval: 60000,
            version: Arc::new(RwLock::new(String::default())),
            storage: std::default::Default::default(),
            data_seeder: None,
            tx: None,
            is_dirty: Arc::new(RwLock::new(false)),
        }
    }
}

fn hash(hm: &HashMap<String, Box<Value>>) -> u64 {
    let mut h: u64 = 0;

    for (k, v) in hm.iter() {
        let mut hasher = XxHash64::default();
        k.hash(&mut hasher);
        v.ts.hash(&mut hasher);
        h ^= hasher.finish();
    }

    h
}

impl Hash for Default {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write_u64(hash(&self.storage.read().unwrap().clone()));
    }
}

/// An implementation of the StateValue trait.
///
/// This value is a serde_json::Value, has a timestamp to resolve conflicts and supports a TTL. See the module
/// documentation for more information.
#[derive(Serialize, Debug, Clone, Deserialize)]
struct Value {
    /// A serde_json::Value to hold any value that can be serialized into JSON format.
    value: serde_json::Value,

    /// The timestamp when this value was first created.
    #[serde(default = "epoch")]
    ts: u64,

    /// An optional TTL (resolved to an absolute epoch time) when this value will be expired.
    ttl: Option<u64>,
}

impl Value {
    /// Returns true if the value was expired.
    fn is_expired(&self) -> bool {
        match self.ttl {
            Some(ttl) => ttl + self.ts < epoch(),
            _ => false,
        }
    }
}

impl StateValue for Value {
    fn as_bytes(&self) -> Option<Vec<u8>> {
        serde_json::to_vec(self).ok()
    }
}

impl StateValue for HashMap<String, Box<Value>> {
    fn as_bytes(&self) -> Option<Vec<u8>> {
        serde_json::to_vec(self).ok()
    }
}

impl StateValue for (String, Value) {
    fn as_bytes(&self) -> Option<Vec<u8>> {
        serde_json::to_vec(self).ok()
    }
}

#[typetag::serde]
impl state::State for Default {
    /// Initializes the state.
    ///
    /// Seeds the state with the specified DataSeeder (if one was specified).
    ///
    /// Spawns an async set thread which will perform async commits to the state.
    ///
    /// Spawns a new thread to purge expired values and versions at a certain interval and returns a safe state
    /// to be shared with the connection and agent layers.
    fn init(&self) -> state::SafeState {
        let mut this = self.clone();

        // if we have a data seeder then use it to seed the data
        this.data_seeder.clone().and_then(|data_seeder| {
            info!("Seeding data...");
            if let Err(e) =  this.seed(data_seeder) {
                warn!("Failed to seed data; ({})", e);
            }

            Some(())
        });

        // start the async_set consumer thread
        let (tx, rx) = mpsc::sync_channel(MAX_SET_OPS);
        this.tx = Some(tx);

        let this = Arc::new(this);
        let t = this.clone();
        tokio::task::spawn_blocking(move || {async_set(t, rx)});
      
        // start the purger thread
        tokio::spawn(purge(this.clone()));

        this
    }

    /// Returns the current state version.
    fn version(&self) -> String {
        if self.is_dirty.read().unwrap().clone() {
            *self.version.write().unwrap() = hash(&self.storage.read().unwrap()).to_string();
            *self.is_dirty.write().unwrap() = false;
        }

        self.version.read().unwrap().clone()
    }

    /// Sets a new value to the state.
    ///
    /// The new value is expected to be in a form of a key/value hashmap.
    /// The value (hashmap) is then filtered to include only values that are new or newer than the current
    /// values in store. This is to resolve conflicts of updating items that were already updated
    /// by another peer. See the module documentation for more information on conflict resolution.
    fn set(&self, value: &dyn StateValue) -> Result<(), Box<dyn StdError>> {
        value.as_bytes().and_then(|value| {
            self.tx.as_ref().and_then(|tx| tx.send(value).ok())
        });

        Ok(())
    }

    /// Returns the value associated with the specified key.
    ///
    /// `key` is expected to resolve to a string.
    fn get(&self, key: &dyn StateValue) -> Option<Box<dyn StateValue>> {
        let key: String = String::from_utf8(key.as_bytes().unwrap_or(Vec::new())).unwrap();

        let storage = self.storage.read().unwrap().clone();
        storage
            .get(&key)
            .cloned()
            .filter(|v| !v.is_expired())
            .map(|v| v.into())
    }

    /// Returns the whole state (root).
    fn get_root(&self) -> Option<Box<dyn StateValue>> {
        let value: HashMap<String, Box<Value>> = self.storage.read().unwrap().clone();
        Some(value.into())
    }

    /// Returns the difference between the current state and `other`.
    ///
    /// If a key is present in both the current state and `other`, it will check if 
    /// the timestamps are equal and if not then it'll include either the current value or 
    /// the one from `other`, based on who's value has the most recent timestamp.
    fn diff(&self, other: &dyn StateValue) -> Result<Box<dyn StateValue>, Box<dyn StdError>> {
        let other: Result<HashMap<String, Box<Value>>, Box<dyn StdError>> = other.into();
        let other = other?;

        let d = self.storage.read().unwrap().clone().difference_with(other, |left, right| {
            if left.ts == right.ts {
                None
            } else {
                Some(if left.ts < right.ts { left } else { right })
            }
        });

        Ok(Box::new(d))
    }
}

/// Purges expired keys at a specficied interval.
fn purge(state: Arc<Default>) -> impl futures::future::Future<Output = ()> + Send {
    let purge_interval = state.purge_interval;

    async move {
        let start = Instant::now() + Duration::from_millis(purge_interval);
        let mut interval = interval_at(start, Duration::from_millis(purge_interval));

        loop {
            interval.tick().await;
            state.purge();
        }
    }
}

/// Async set thread.
///
/// Listens on the receiver channel for values to be commited to the state.
fn async_set(state: Arc<Default>, rx: mpsc::Receiver<Vec<u8>>) {
    for value in rx.iter() {
        let value = serde_json::from_slice(value.as_slice());
        if let Ok(value) = value {
            Default::set(&state, &value);
        }
    }
}

impl From<&dyn StateValue> for Result<HashMap<String, Box<Value>>, Box<dyn StdError>> {
    fn from(value: &dyn StateValue) -> Self {
        let value: Vec<u8> = value
            .as_bytes()
            .ok_or(Vec::from("")).unwrap();

        serde_json::from_slice(value.as_slice()).map_err(|e| e.into())
    }
}

impl From<Box<Value>> for Box<dyn StateValue> {
    fn from(value: Box<Value>) -> Self {
        Box::new(*value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::State;

    #[test]
    fn state_versions_should_be_equal() {
        let value = HashMap::unit("cat".to_string(), Value {value: "garfield".into(), ts: 0, ttl: None}.into());

        let first = Default::default();
        let second = Default::default();

        first.set(&value);
        second.set(&value.clone());

        assert_eq!(first.version(), second.version());
    }

    #[test]
    fn state_versions_should_be_different() {
        let value1 = HashMap::unit("cat".to_string(), Value {value: "garfield".into(), ts: 0, ttl: None}.into());
        let value2 = HashMap::unit("cat".to_string(), Value {value: "garfield".into(), ts: 1, ttl: None}.into());

        let first = Default::default();
        let second = Default::default();

        first.set(&value1);
        second.set(&value2);

        assert_ne!(first.version(), second.version());
    }

    #[test]
    fn should_purge_items() {
        let value = HashMap::unit("dog".to_string(), Value {value: "snoopy".into(), ts: 0, ttl: None}.into());

        let state = Default::default();

        // Force insersion of expired vlaue
        state.storage.write().unwrap().insert("cat".to_string(), Value {value: "garfield".into(), ts: 0, ttl: Some(1)}.into());
        state.set(&value);

        assert_eq!(state.storage.read().unwrap().len(), 2);
        state.purge();
        assert_eq!(state.storage.read().unwrap().len(), 1);
    }

    #[test]
    fn should_not_return_expired_values() {
        let value = HashMap::unit("dog".to_string(), Value {value: "snoopy".into(), ts: 0, ttl: None}.into());

        let state = Default::default();

        // Force insersion of expired vlaue
        state.storage.write().unwrap().insert("cat".to_string(), Value {value: "garfield".into(), ts: 0, ttl: Some(1)}.into());
        state.set(&value);

        assert!(state.get(&"dog".to_string() as &dyn StateValue).is_some());
        assert!(state.get(&"cat".to_string() as &dyn StateValue).is_none());
    }

    #[test]
    fn should_be_marked_as_dirty() {
        let value = HashMap::unit("dog".to_string(), Value {value: "snoopy".into(), ts: 0, ttl: None}.into());
        let state = Default::default();

        assert_eq!(*state.is_dirty.read().unwrap(), false);
        state.set(&value);
        assert_eq!(*state.is_dirty.read().unwrap(), true);
    }

    #[test]
    fn should_return_the_whole_state() {
        let value1 = HashMap::unit("cat".to_string(), Value {value: "garfield".into(), ts: 0, ttl: None}.into());
        let value2 = HashMap::unit("dog".to_string(), Value {value: "snoopy".into(), ts: 0, ttl: None}.into());
        let state = Default::default();

        state.set(&value1);
        state.set(&value2);

        let hm: Result<HashMap<String, Box<Value>>, Box<dyn StdError>> = (&*state.get_root().unwrap()).into();
        let mut keys: Vec<String> = hm.unwrap().keys().cloned().collect();

        assert_eq!(vec!("cat", "dog").sort(), keys.sort());
    }

    #[test]
    fn should_return_diff() {
        let value1 = HashMap::unit("cat".to_string(), Value {value: "garfield".into(), ts: 0, ttl: None}.into());
        let value2 = HashMap::unit("dog".to_string(), Value {value: "snoopy".into(), ts: 0, ttl: None}.into());

        let state = Default::default();

        state.set(&value1);
        state.set(&value2);

        let diff: Result<HashMap<String, Box<Value>>, Box<dyn StdError>> = (&*state.diff(&value2).unwrap()).into();
        let keys: Vec<String> = diff.unwrap().keys().cloned().collect();

        assert_eq!(vec!("cat"), keys);
    }
}
