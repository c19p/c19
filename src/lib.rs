//! # The C19 Protocol
//!
//! A variant of the [Gossip protocol](https://en.wikipedia.org/wiki/Gossip_protocol) that allows a
//! group of services to agree on a service-wide state.
//!
//! The state is shared across a distributed set of services which in effect means that each
//! service has the data available locally.
//!
//! The C19 protocol decouples the process of fetching the data from using it.
//!
//! Consider a service which depends on another service to get subscription data for a user. Many
//! considerations should be taken into account on the availability of the other service. The
//! dependent service is responsible for both retrieving the data and using it, but retrieving the
//! data should never be its main focus. Handling errors and cases where the other service fails to
//! respond is a cognitive overhead and not the main responsibility of the dependent.
//!
//! Services like Istio exist to solve this problem. But nothing beats having the data available
//! locally. By decoupling the process of fetching the data from using it, the service can focus on
//! using the data and have C19 handle the fetching.
//!
//! C19 is a simple, powerful and extensible system that gurantees reliability, availability, efficiency
//! and low resource footprint. It does so by having an agent attached to each service and exchange
//! its state in an efficient manner with other c19 agents, effectively making sure the data is
//! available locally to the service.
//!
//! # The Three Layers
//! C19 is built upon three different layers: The Agent, State and Connection layers.
//!
//! Many different types of each layer can be implemented and can work in conjuction with the other
//! layers. Depending on your usage, you can choose the right layer that suits your needs.
//!
//! ## The Agent Layer
//! The agent layer is responsible for communicating with your app. It allows your app to set and
//! get values to and from the state.
//!
//! ## The State Layer
//! The state layer is responsible for holding the data and to manage it. It exposes an API for the
//! Agent and Connection layers to set and get values from and to it. For example, the
//! `Defaul` state layer implementation is a Key/Value store that supports TTL for values.
//!
//! Different state layers can be implemented to handle data in different ways. One might handle
//! data of files and versions likes Git. It doesn't have to be a Key/Value store.
//!
//! ## The Connection Layer
//! The connection layer is responsible for spreading the state (data) across the system by
//! connecting to other agents (peers). 
//!
//! Different connection layers can be implemented to support different protocols and handle the
//! data in different ways. One implementation might handle big data in a more efficient way than
//! another implementation.
//!
//! # What to Consider When Using the C19 Protocol
//! #### 1. When data changes at a high rate and you need to access the most recent change in realtime.
//!
//! In the example above, you can imagine subscription data does not change so often and when it
//! does, it's ok to get the change after a second. If you need immediate access to the most recent 
//! changes and cannot bare a stale date, then you should consider a different option. The system is a 
//! distributed system with an eventual consistency.  The rate in which the data is being propagated 
//! throughout the system depends on your configuration and the implementation of the different layers, 
//! but it's not strict consistency.
//!
//! C19 should answer most use-cases as mostly systems can tolerate data that is near-realtime or stale.
//!
//! #### 2. When your data is very large and changes very often.
//!
//! There's no limit set in the code. It's up to you to decide the limit of your data. 
//! The data is held in memory and being exchanged over the network between peers. You
//! should consider those two parameters when you decide on your data limits and the different
//! layers you choose to use.
//!
//! # Who This Documentation Is For?
//! C19 has been designed to be easily extensible. One could implement different strategies for
//! exhanging state between peers, for holding data in memory, for applying different algorithms
//! for state management, etc.
//!
//! This documentation is about that. It's a walkthrough of the code so that you can find your way
//! around and extend the protocol as needed.
//!
//! If you'd like to learn more about using C19 as it is, please read the user-guide book. [FIXME: link to
//! user-guide book]
//!
//! Accompaning this documentation is the book about the high level design and some ideas on how to
//! extend the protocol. Read more about it here. [FIXME: link to the developer guide]
//!
//! # Kubernetes
//! While the C19 protocol can be used anywhere, it was design to be Kubernetes first. This means
//! that you will find different deployment strategies, peer providers and an all-in-all mindset of
//! Kubernetes. One of the goals of the project is to "Just work" and to allow a user of the
//! project a fast and easy-to-reason-about deployment to a Kubernetes cluster.
//!
mod agent;
mod connection;
mod helpers;
mod state;

pub mod config;

use futures::future::Future;
use futures::stream::FuturesUnordered;
use std::clone::Clone;
use std::sync::Arc;
use std::error::Error as StdError;

/// Initializes the state and runs the connection and agent layers.
///
/// The state is given a chance to be initialized by running state::init
/// on the instance. The connection and agent layers are then started while
/// given the initialized state.
///
/// The instances for the state, connection and agent are the ones
/// initialized by the configuration.
///
/// The connection and agents layers are expected to return a future
/// which is then being waited on until completion (mostly indfefinately)./
pub fn run(config: config::Config) -> impl Future<Output = Result<(), Box<dyn StdError + Send + Sync>>> {
    let state = config.spec.state.init();
    let conn = Arc::new(config.spec.connection).clone();
    let agent = Arc::new(config.spec.agent).clone();

    let mut futures = FuturesUnordered::new();
    let state1 = state.clone();
    futures.push(tokio::spawn(async move { conn.start(state1).await }));

    let state2 = state.clone();
    futures.push(tokio::spawn(async move { agent.start(state2).await }));

    async move {
        let mut iter = futures.iter_mut();
        while let Ok(result) = iter.next().unwrap().await {
            if let Err(e) = result {
                return Err(e);
            }
        }

        Ok(())
    }
}
