use wstd::http::Request;
use wstd::http::body::IncomingBody;
use wstd::http::server::{Finished, Responder};
use wstd::http::{Response, StatusCode};
use wstd::io::empty;

#[wstd::http_server]
async fn main(_request: Request<IncomingBody>, responder: Responder) -> Finished {
    let response = Response::builder()
        .status(StatusCode::OK)
        .body(empty())
        .unwrap();
    responder.respond(response).await
}
