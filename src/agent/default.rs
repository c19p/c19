//! Default implementation of the Agent trait.
//!
//! This is a simple yet decent implementation of the Agent trait and should answer most use
//! cases.  
//!
//! This agent exposes a GET and a PUT endpoints to allow an app to get and set values from and to the
//! state.
//!
//! The format of the keys are dependent on the `State` layer being used. For example, when using
//! the [Default] state, the keys are expected to
//! be a String and the values are expected to be in the format specified in the documentation of
//! the state layer.
//!
//! # GET /`<key>`
//! To get a value, the app can send a `GET` request with a `key` that represents a key in the
//! state. 
//!
//! Expects key to be a String. Returns the value as-is from the state.
//!
//! # Example
//!
//! Assuming usage of the [Default] state, a value might look like this:
//!
//! ```
//! GET /cat
//!
//! {"ts":1601241450390,"ttl":null,"value":"garfield"}
//! ```
//!
//! As you can see, the value holds more than the value itself. Included are the ttl (can be null)
//! which is an absolute value of when this key will expire, and the timestamp (ts) that this key
//! was created.
//!
//! A different form of the value might be returned, depending on which state layer is being used.
//! In any case, this agent implementation does not assume anything about the format of the values
//! returned by the state.
//! # PUT /
//! To set a value to the state, the app can send a `PUT` request with a body that conforms to the
//! state expected value.
//!
//! The agent does not assume anything about the format of the value in the body of the message.
//! The value is passed to the state as-is and it is up to the app to make sure it conforms to the 
//! expected format by the state.
//!
//! For example, using the [Default] state, an app would send a `PUT` request like so:
//!
//! A JSON body of the followibng format:
//!
//! ```
//! {"cat": {"value": "garfield", "ttl": 60000}}
//! ```
//!
//! According to the [Default] state documentation, the value can be anything JSON.
//! `ttl` is optional and another field `ts` can be specified to override the automatic `ts` 
//! set by the state.
//!
//! Please refer to the [Default] state implementation for more information. 
//!
//! [Default]: state::default

use crate::agent;
use crate::helpers::http::responses::Responses;
use crate::state::{self, StateValue};
use futures::future::{BoxFuture, FutureExt, TryFutureExt};
use http::{Request, Response};
use hyper::{http::Method, service::make_service_fn, service::service_fn, Body, Server};
use serde::{Deserialize, Serialize};
use std::error::Error as StdError;

type Result<T> = std::result::Result<T, Box<dyn StdError + Send + Sync>>;

/// The Default struct.
///
/// This struct holds information loaded from the agent configuration.
#[derive(Serialize, Deserialize, Debug)]
#[serde(default)]
pub struct Default {
    /// Binds and accepts connections on this port.
    /// Default port: 3097
    port: u16,
}

/// Default values for this implementation.
impl std::default::Default for Default {
    fn default() -> Self {
        Default { port: 3097 }
    }
}

/// Returns the value associated with the given key.
///
/// `GET /<key>`
///
/// Expects key to be a String. Returns the value as-is from the state.
///
/// # Example
///
/// Assuming usage of the `Default` state, a value might look like this:
///
/// ```
/// GET /cat
///
/// {"ts":1601241450390,"ttl":null,"value":"garfield"}
/// ```
///
/// As you can see, the value holds more than the value itself. Included are the ttl (can be null)
/// which is an absolute value of when this key will expire, and the timestamp (ts) that this key
/// was created.
///
/// A different form of the value might be returned, depending on which state layer is being used.
/// In any case, this agent implementation does not assume anything about the format of the values
/// returned by the state.
fn get_handler(state: state::SafeState, req: &Request<Body>) -> Response<Body> {
    let result = req.uri().path().split('/').last().and_then(|key| {
        state
            .get(&key.to_string() as &dyn state::StateValue)
            .and_then(|value| Some(value.as_bytes()))
    });

    match result {
        Some(value) => {
            if let Some(value) = value {
                Responses::ok(value.into())
            } else {
                Responses::bad_request(None)
            }
        }
        _ => Responses::not_found(None),
    }
}

/// Sets the key and value specified in the request.
///
/// `PUT /`
///
/// The agent does not assume anything about the format of the value in the body of the message.
/// The value is passed to the state as-is and it is up to the app to make sure it conforms to the 
/// expected format by the state.
///
/// For example, using the `Default` state, an app would send a `PUT` request like so:
///
/// A JSON body of the followibng format:
///
/// ```
/// {"cat": {"value": "garfield", "ttl": 60000}}
/// ```
///
/// According to the `Default` state documentation, the value can be anything JSON.
/// `ttl` is optional and another field `ts` can be specified to override the automatic `ts` 
/// set by the state.
///
/// Please refer to the [Default] state implementation for more information. 
///
/// [Default]: crate::state::default::Default
fn set_handler(
    state: state::SafeState,
    req: Request<Body>,
) -> impl FutureExt<Output = Result<Response<Body>>> {
    hyper::body::to_bytes(req.into_body()).and_then(move |body| async move {
        let result = state.set(&body as &dyn StateValue);

        Ok(match result {
            Ok(_) => Responses::no_content(),
            _ => Responses::unprocessable(None),
        })
    }).map_err(|e| e.into())
}

/// Accepts a request and dynamically dispatches the handler based on the method of the request.
///
/// Returns whatever the get and set handlers return or 404 (not found) if method is invalid.
async fn handler(state: state::SafeState, req: Request<Body>) -> Result<Response<Body>> {
    Ok(match req.method() {
        &Method::GET => get_handler(state, &req),
        &Method::PUT => set_handler(state, req).await.unwrap(),
        _ => Responses::not_found(None),
    })
}

impl Default {
    async fn server(&self, state: state::SafeState) -> Result<()> {
        let service = make_service_fn(move |_| {
            let state = state.clone();
            async move {
                Ok::<_, Box<dyn StdError + Send + Sync>>(service_fn(move |req| {
                    handler(state.clone(), req)
                }))
            }
        });

        let server = Server::try_bind(&([0, 0, 0, 0], self.port).into())?;
        server.serve(service).await?;

        Ok(())
    }
}

#[typetag::serde]
impl agent::Agent for Default {
    /// Starts the server while passing the current state to be used by the handlers.
    fn start<'a>(&'a self, state: state::SafeState) -> BoxFuture<'a, Result<()>> {
        self.server(state).map_err(|e| e.into()).boxed()
    }
}
