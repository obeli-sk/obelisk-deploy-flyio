use wstd::http::Body;
use wstd::http::Request;
use wstd::http::Response;

#[wstd::http_server]
async fn main(_request: Request<Body>) -> Result<Response<Body>, wstd::http::Error> {
    Ok(Response::new("Healthcheck OK!\n".into()))
}
