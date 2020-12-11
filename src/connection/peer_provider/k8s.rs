//! An implementation of the peer provider.
//!
//! This implementation targets Kubernetes. It queries the Kubernetes api server for endpoints of
//! pods based on the `selector` configuration.
//!
//! Watches the list of pods for any change and updates a local vector of peers so when the
//! conncetion implementation queries the peer provider, it returns the full vector.
//!
//! # Example:
//!
//! ```yaml
//!    peer_provider:
//!      kind: K8s
//!      selector:
//!        c19: example
//!      namespace: :all
//! ```

use crate::connection::peer_provider::{PeerProvider, Peer};
use futures::{StreamExt, TryStreamExt};
use k8s_openapi::api::core::v1::Pod;
use kube::{
    api::{Api, ListParams, Meta, WatchEvent},
    Client,
};
use log::error;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::error::Error as StdError;

type Result<T> = std::result::Result<T, Box<dyn StdError + Send + Sync>>;

#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct K8s {
    /// Label selector to query for pods.
    /// This is loaded from the peer provider configuration.
    /// Defaults to empty string (all pods).
    selector: HashMap<String, String>,

    /// The namespace to scope the query.
    /// Default value is the "default" namesapce.
    /// If set to `:all` then all namespaces will be queried.
    namespace: String,

    /// Holds the updated list of peer endpoints.
    #[serde(skip_serializing, skip_deserializing)]
    peers: Arc<RwLock<HashMap<String, Peer>>>,
}

impl std::default::Default for K8s {
    fn default() -> Self {
        K8s {
            selector: Default::default(),
            namespace: "default".to_string(),
            peers: Default::default(),
        }
    }
}

impl K8s {
    /// Returns the selector as a formated string to match the format expected by the Kubernetes
    /// API.
    fn selector(&self) -> String {
        self.selector
            .iter()
            .fold(String::new(), |s, (k, v)| format!("{},{}={}", s, k, v))
            .strip_prefix(",")
            .unwrap_or("")
            .to_string()
    }

    /// Returns the IP of the specified pod.
    fn ip(pod: &Pod) -> Option<Peer> {
        pod.status.as_ref().and_then(|status| {
            status
                .pod_ip
                .as_ref()
                .and_then(|ip| ip.parse().ok())
        })
    }
}

#[typetag::serde]
impl PeerProvider for K8s {
    /// Initializes the peer provider.
    ///
    /// Spawns a `watch` thread that will track changes to the list of pods.
    fn init(&self) -> Result<()> {
        let selector = self.selector();
        let peers = self.peers.clone();
        let namespace = self.namespace.clone();

        tokio::spawn(async move {
            let client = Client::try_default().await?;
            let pods: Api<Pod> = if namespace == ":all" {
                Api::all(client)
            } else {
                Api::namespaced(client, namespace.as_ref())
            };
            let lp = ListParams::default().labels(&selector);
            let mut events = pods.watch(&lp, "0").await?.boxed();

            while let Some(event) = events.try_next().await? {
                let event = &event;
                match event {
                    WatchEvent::Added(pod) | WatchEvent::Modified(pod) => {
                        if let Some(ip) = K8s::ip(pod) {
                            peers.write().unwrap().insert(
                                Meta::meta(pod).uid.as_ref().unwrap().clone(),
                                ip,
                            );
                        }
                    }
                    WatchEvent::Deleted(pod) => {
                        peers
                            .write()
                            .unwrap()
                            .remove(Meta::meta(pod).uid.as_ref().unwrap());
                    }
                    _ => error!("Some error occured while receiving pod event"),
                }
            }
            Ok::<_, kube::Error>(())
        });
        Ok(())
    }

    /// Returns the vector of peers.
    fn get(&self) -> Vec<Peer> {
        self.peers
            .read()
            .unwrap()
            .values()
            .map(|value| value.clone())
            .collect()
    }
}
