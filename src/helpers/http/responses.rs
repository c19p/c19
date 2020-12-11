use http::Response;
use hyper::{Body, StatusCode};

pub struct Responses {}

impl Responses {
    pub fn response(status: StatusCode, body: Body) -> Response<Body> {
        Response::builder().status(status).body(body).unwrap()
    }

    pub fn ok(body: Body) -> Response<Body> {
        Responses::response(StatusCode::OK, body)
    }

    pub fn not_found(body: Option<Body>) -> Response<Body> {
        Responses::response(StatusCode::NOT_FOUND, body.unwrap_or("not found".into()))
    }

    pub fn bad_request(body: Option<Body>) -> Response<Body> {
        Responses::response(
            StatusCode::BAD_REQUEST,
            body.unwrap_or("bad request".into()),
        )
    }

    pub fn no_content() -> Response<Body> {
        Responses::response(StatusCode::NO_CONTENT, Body::empty())
    }

    pub fn unprocessable(body: Option<Body>) -> Response<Body> {
        Responses::response(
            StatusCode::UNPROCESSABLE_ENTITY,
            body.unwrap_or("malformed input".into()),
        )
    }

    pub fn internal_error(body: Option<Body>) -> Response<Body> {
        Responses::response(
            StatusCode::INTERNAL_SERVER_ERROR,
            body.unwrap_or("internal error".into()),
        )
    }
}
