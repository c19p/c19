//! The default connection layer implementation
//!
//! This is an implementation of the Connection trait.
//!
//! It offers a simple and powerful way of exchanging data with other peers.
//!
//! # Choosing peers
//! The connection chooses peers by using a [peer provider]. A peer provider is an abstration over a
//! vector of peers. A peer provider is responsible for retrieving the full list of peers this
//! connection layer can connect with. See the [peer provider] documentation for more information.
//!
//! If not provided in the configuration, this connection will choose the `k8s` peer provider to
//! get the full list of available peers to connect to. The `k8s` peer provider queries Kubernetes
//! server api for endpoints of other peers by using the selector field in the configuration.
//!
//! The default connection will then randomly select a subset of the peers and will exchange the
//! state with each one of them in parallel. The number of peers sampled from the full list is
//! determined by the `r0` configuration field.
//!
//! # Exchanging data
//! The connection uses HTTP to push and pull (PUT and GET) the state to and from other peers.
//!
//! ## Pushing
//! Only the changes from the last publish time will be pushed to other peers.
//!
//! ## Pulling
//! When pulling, the connection layer specifies its own state version and if it matches the one 
//! that the other peer has then nothing will be exchanged. If the versions do not match then 
//! the other peer will return its own full state.
//!
//! The connection layer does not assume anything about the content of the data being exchanged.
//! The data will be passed as-in to other peers.
//!
//! The interval in which the data will be exchanged is set in the `push_interval` and `pull_interval` configuration flags.
//!
//! See more about the default implementation and the different options it provides in the [struct documentation].
//!
//! [peer provider]: connection::peer_provider
//! [struct documentation]: struct@Default

use crate::connection;
use crate::connection::peer_provider;
use crate::helpers::http::responses::Responses;
use crate::helpers::middlewares::json::wrap_json_response;
use crate::helpers::utils::Sample;
use crate::state;
use futures::future::{self, BoxFuture, FutureExt, TryFutureExt};
use futures::{stream, StreamExt};
use hyper::{
    http::Method, service::make_service_fn, service::service_fn, Body, Request, Response, Server,
};
use log::{debug, warn};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::convert::Infallible;
use std::time::Duration;
use tokio;
use tokio::time;
use std::error::Error as StdError;

type Result<T> = std::result::Result<T, Box<dyn StdError + Send + Sync>>;

#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Default {
    /// The port to bind to and listen for connection from other peers.
    /// Default port: 4097.
    port: u16,

    /// An optional port to use as a target when sending the state to peers. If ommited,
    /// then the `port` field will be used both for accepting connections and when connecting to
    /// other peers. Default is null (use the `port` field).
    target_port: Option<u16>,

    /// The publish interval in milliseconds.
    /// Default value is 1 second (1000 milliseconds).
    ///
    /// The connection layer will publish a new state `push_interval` after it has 
    /// finished publishing the previous state. This means that if preparing a state for publishing 
    /// takes a few seconds, then only after those few seconds the counter will start counting
    /// `push_interval`.
    push_interval: u64,

    /// The pull interval in milliseconds.
    /// Default value is 60 seconds (60000 milliseconds).
    ///
    /// The connection layer will connect and pull the state from peers every `pull_interval`.
    pull_interval: u64,

    /// The number of peers to connect to on each interval and exchange the state with.
    /// Default value: 3.
    r0: usize,

    /// The connection timeout when connecting and exchanging data with a peer.
    /// Default value: 1000ms. 
    timeout: u64,

    /// The peer provider to use.
    ///
    /// The connection layer will reach out to the peer provider to get the full list of available
    /// peers to choose from.
    ///
    /// Default value: k8s.
    pub peer_provider: Box<dyn peer_provider::PeerProvider>,
}

impl std::default::Default for Default {
    fn default() -> Self {
        Default {
            port: 4097,
            target_port: None,
            push_interval: 1000,
            pull_interval: 60000,
            r0: 3,
            timeout: 1000,
            peer_provider: Box::new(peer_provider::k8s::K8s::default()),
        }
    }
}

/// Returns an HTTP response with the full state as JSON.
///
/// GET /
fn get_handler(state: state::SafeState, req: &Request<Body>) -> Response<Body> {
    let versions_match = req.uri().path().split('/').last().and_then(|version| {
        (version.is_empty() || version != state.version()).into()
    }).unwrap();

    if versions_match {
        Responses::ok(state.get_root().unwrap_or("".into()).into())
    } else {
        Responses::no_content()
    }
}

/// Accepts a JSON body that represents a state. Merges it with its own state.
///
/// PUT /
fn set_handler<'a>(
    state: state::SafeState,
    req: Request<Body>,
) -> impl FutureExt<Output = Result<Response<Body>>> + 'a {
    hyper::body::to_bytes(req.into_body())
        .and_then(move |body| async move {
            let body = &body as &dyn state::StateValue;
            let result = state.set(body);

            Ok(match result {
                Ok(_) => Responses::no_content(),
                _ => Responses::unprocessable(None),
            })
        })
        .map_err(|e| e.into())
}

async fn handler(state: state::SafeState, req: Request<Body>) -> Result<Response<Body>> {
    Ok(match req.method() {
        &Method::GET => get_handler(state, &req),
        &Method::PUT => set_handler(state, req).await.unwrap_or(Responses::internal_error(Some("Failed to commit state sent by remote peer.".into()))),
        _ => Responses::not_found(None),
    })
}

impl Default {
    async fn server(&self, state: state::SafeState) -> Result<()> {
        let service = make_service_fn(move |_| {
            let state = state.clone();
            async move {
                Ok::<_, Infallible>(service_fn(move |req| {
                    wrap_json_response(handler)(state.clone(), req)
                }))
            }
        });

        let server = Server::try_bind(&([0, 0, 0, 0], self.port).into())?;
        server.serve(service).await?;

        Ok(())
    }

    /// Receiver thread.
    /// Connects to `r0` peers at random and pulls their state into its own state.
    ///
    /// When pulling the state, the version of the current state is specified in the request 
    /// path so the other peer would determine whether to respond with its full state or nothing.
    ///
    /// If the current version matches the one of the peer's version then the peer will respond 
    /// with 204 (no content). The full state will be returned by the peer if the versions do not
    /// match.
    async fn receiver(&self, state: state::SafeState) -> Result<()> {
        loop {
            // sample r0 peers
            let peers = self.peer_provider.get();
            let peers = peers.into_iter().sample(self.r0);
          
            let res = stream::iter(peers)
                .map(|peer| {
                    let url = format!("http://{}:{}/{}", peer.ip(), peer.port().unwrap_or(self.target_port.unwrap_or(self.port)), state.version());
                    let timeout = self.timeout;

                    tokio::spawn(async move {
                        let client = Client::builder()
                            .connect_timeout(Duration::from_millis(timeout))
                            .build().unwrap();

                        let result = client
                            .get(&url.to_string())
                            .send()
                            .await?
                            .bytes()
                            .await;

                        result
                    })
                })
                .buffer_unordered(4);

            res.collect::<Vec<_>>().await.iter().for_each(|result| {
                if let Ok(Ok(result)) = result {
                    if let Err(e) = state.set(result as &dyn state::StateValue) {
                        warn!("Failed to set peer response to state; {}", e);
                    }
                }
            });

            time::delay_for(time::Duration::from_millis(self.pull_interval)).await;
        }
    }

    /// Publisher thread.
    /// Publishes the state at an `interval` time.
    ///
    /// This thread will loop forever, waiting for `interval` time to pass.
    /// It will then reach out to the peer provider to get the full list of available peers and
    /// will randomly pick `r0` peers to exchange the state with.
    ///
    /// It connects to other peers in parallel and publishes the changes since last publish.
    async fn publisher(&self, state: state::SafeState) -> Result<()> {
        let mut last_published: Vec<u8> = Vec::<u8>::default();
        let mut last_published_version = String::default();

        loop {
            if last_published_version == state.version() {
                time::delay_for(time::Duration::from_millis(self.push_interval)).await;
                continue;
            }

            let last = last_published.clone();
            let state_clone = state.clone();
            let res = tokio::task::spawn_blocking(move || {
                // get the recent state
                let root = state_clone.get_root().unwrap_or("".into()).as_bytes().unwrap();

                if root == last {
                    return None;
                }

                let state_to_publish = state_clone.diff(&last).and_then(|diff| {
                    Ok(diff.as_bytes().unwrap())
                }).or::<Vec<u8>>(Ok(root.clone())).unwrap();

                Some((state_to_publish, root))
            }).await?;

            if res.is_none() {
                time::delay_for(time::Duration::from_millis(self.push_interval)).await;
                continue;
            }

            let (state_to_publish, last) = res.unwrap();
            last_published = last;
            last_published_version = state.version();

            // sample r0 peers
            let peers = self.peer_provider.get();
            let peers = peers.into_iter().sample(self.r0);

            // start sending to peers in parallel
            let res = stream::iter(peers)
                .map(|peer| {
                    let state_to_publish = state_to_publish.clone();
                    let url = format!("http://{}:{}/", peer.ip(), peer.port().unwrap_or(self.target_port.unwrap_or(self.port)));
                    let timeout = self.timeout;

                    tokio::spawn(async move {
                        let client = Client::builder()
                            .connect_timeout(Duration::from_millis(timeout))
                            .build().unwrap();

                        debug!("Publishing state to {}", url);
                        let result = client
                            .put(&url.to_string())
                            .body(state_to_publish)
                            .send()
                            .await?
                            .bytes()
                            .await;

                        result
                    })
                })
                .buffer_unordered(4);

            res.collect::<Vec<_>>().await.iter().for_each(|result| {
                if let Ok(Err(e)) = result {
                    warn!("Failed to publish to peer; {}", e);
                }
            });

            time::delay_for(time::Duration::from_millis(self.push_interval)).await;
        }
    }
}

#[typetag::serde]
impl connection::Connection for Default {
    /// Starts the connection layer.
    ///
    /// Initialize the peer provider, the server and the publisher thread and returns a future that
    /// runs them all.
    fn start<'a>(&'a self, state: state::SafeState) -> BoxFuture<'a, Result<()>> {
        let init: Result<()> = self.peer_provider.init().map_err(|e| e.into());
        let init = async { Ok(init) };

        let server = self.server(state.clone());
        let publisher = self.publisher(state.clone());
        let receiver = self.receiver(state.clone());

        future::try_join4(init, server, publisher, receiver)
            .map_ok(|_| ())
            .boxed()
    }
}

impl state::StateValue for Vec<u8> {
    fn as_bytes(&self) -> Option<Vec<u8>> {
        Some(self.to_owned())
    }
}

impl state::StateValue for hyper::body::Bytes {
    fn as_bytes(&self) -> Option<Vec<u8>> {
        Some(self.to_vec())
    }
}

impl From<Box<dyn state::StateValue>> for hyper::Body {
    fn from(value: Box<dyn state::StateValue>) -> Self {
        value.as_bytes().unwrap_or("{}".into()).into()
    }
}

impl From<Box<dyn state::StateValue>> for reqwest::Body {
    fn from(value: Box<dyn state::StateValue>) -> Self {
        value.as_bytes().unwrap_or("{}".into()).into()
    }
}
