use crate::generated::exports::obelisk_client::api_http::executions::{
    ExecutionId, ExecutionWithState, Guest,
};
use anyhow::anyhow;
use generated::export;
use serde::Serialize;
use serde_json::Value;
use ulid::Ulid;
use wstd::{
    http::{Body, Client, Method, Request},
    runtime::block_on,
};

mod generated {
    #![allow(clippy::empty_line_after_outer_attr)]
    include!(concat!(env!("OUT_DIR"), "/any.rs"));
}

struct Component;
export!(Component with_types_in generated);

async fn list(endpoint_url: &str) -> Result<Vec<ExecutionWithState>, anyhow::Error> {
    let client = Client::new();
    let request = Request::builder()
        .uri(format!("{endpoint_url}/v1/executions"))
        .method(Method::GET)
        .header("accept", "application/json")
        .body(Body::empty())?;
    let response = client.send(request).await?;
    eprintln!("< {:?} {}", response.version(), response.status());
    for (key, value) in response.headers().iter() {
        let value = String::from_utf8_lossy(value.as_bytes());
        eprintln!("< {key}: {value}");
    }
    let body: Vec<ExecutionWithState> = response.into_body().json().await?;
    Ok(body)
}

async fn submit(
    endpoint_url: &str,
    execution_id: ExecutionId,
    ffqn: String,
    params: &str,
) -> Result<(), anyhow::Error> {
    #[derive(Serialize)]
    struct RequestPayload {
        ffqn: String,
        params: Value,
    }
    let params = serde_json::from_str(params)?;
    let payload = RequestPayload { ffqn, params };
    let client = Client::new();
    let request = Request::builder()
        .uri(format!("{endpoint_url}/v1/executions/{execution_id}"))
        .method(Method::PUT)
        .header("content-type", "application/json")
        .header("accept", "application/json")
        .body(Body::from_json(&payload)?)?;
    let response = client.send(request).await?;
    let status = response.status();
    if status.is_success() {
        Ok(())
    } else {
        let mut response = response.into_body();
        let resp = response.str_contents().await?;
        Err(anyhow!("status:{}, resp:{resp}", status))
    }
}

impl Guest for Component {
    fn generate() -> Result<ExecutionId, ()> {
        let ulid = Ulid::new();
        Ok(format!("E_{ulid}"))
    }

    fn list(endpoint_url: String) -> Result<Vec<ExecutionWithState>, String> {
        block_on(async move { list(&endpoint_url).await.map_err(|err| err.to_string()) })
    }

    fn submit(
        endpoint_url: String,
        execution_id: ExecutionId,
        ffqn: String,
        params: String,
    ) -> Result<(), String> {
        block_on(async move {
            submit(&endpoint_url, execution_id, ffqn, &params)
                .await
                .map_err(|err| err.to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::{Component, generated::exports::obelisk_client::api_http::executions::Guest as _};

    #[ignore]
    #[test]
    fn list() {
        let url = std::env::var("TEST_ENDPOINT_URL").expect("TEST_ENDPOINT_URL must be set");
        let body = Component::list(url).unwrap();
        println!("{body:?}");
    }

    #[test]
    fn generate() {
        let eid = Component::generate().unwrap();
        println!("{eid}");
    }
}
