use crate::state;
use futures::future::{BoxFuture, Future, FutureExt, TryFutureExt};
use http::{Request, Response};
use hyper::{http::header::HeaderValue, Body};
use std::error::Error as StdError;

type Result<T> = std::result::Result<T, Box<dyn StdError + Send + Sync>>;

pub fn wrap_json_response<F, S: 'static>(
    f: F,
) -> impl Fn(state::SafeState, Request<Body>) -> BoxFuture<'static, Result<Response<Body>>>
where
    F: Fn(state::SafeState, Request<Body>) -> S,
    S: Future<Output = Result<Response<Body>>> + Send,
{
    move |state: state::SafeState, req: Request<Body>| {
        {
            f(state, req).and_then(|mut response: Response<Body>| async {
                response
                    .headers_mut()
                    .insert("Content-Type", HeaderValue::from_static("application/json"));

                Ok(response)
            })
        }
        .boxed()
    }
}
