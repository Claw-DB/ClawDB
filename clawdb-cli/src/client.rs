use reqwest::{Client, Method, Response, StatusCode};
use serde::{de::DeserializeOwned, Serialize};
use std::time::Duration;

use crate::error::{CliError, CliResult};
use crate::types::ApiErrorBody;

pub struct ClawDBClient {
    base_url: String,
    token: Option<String>,
    http: Client,
}

const RETRY_DELAYS: [Duration; 3] = [
    Duration::from_millis(200),
    Duration::from_millis(400),
    Duration::from_millis(800),
];

impl ClawDBClient {
    pub fn new(base_url: String, token: Option<String>) -> CliResult<Self> {
        let http = Client::builder()
            .user_agent(concat!("clawdb-cli/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| CliError::Other(e.to_string()))?;
        Ok(Self {
            base_url,
            token,
            http,
        })
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base_url.trim_end_matches('/'), path)
    }

    /// GET a JSON resource.
    pub async fn get<T: DeserializeOwned>(&self, path: &str) -> CliResult<T> {
        self.retry_json(Method::GET, path, None::<&serde_json::Value>)
            .await
    }

    /// GET with URL query parameters.
    pub async fn get_q<T: DeserializeOwned, Q: Serialize + ?Sized>(
        &self,
        path: &str,
        params: &Q,
    ) -> CliResult<T> {
        let url = self.url(path);
        let mut attempt = 0usize;
        loop {
            let mut req = self.http.get(&url);
            if let Some(t) = &self.token {
                req = req.bearer_auth(t);
            }
            req = req.query(params);
            match send_and_parse::<T>(req).await {
                Ok(v) => return Ok(v),
                Err(ref e) if Self::is_retryable(e) && attempt < RETRY_DELAYS.len() => {
                    tokio::time::sleep(RETRY_DELAYS[attempt]).await;
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// POST a JSON body and receive a JSON response.
    pub async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        body: &B,
    ) -> CliResult<T> {
        let val = serde_json::to_value(body)?;
        self.retry_json(Method::POST, path, Some(&val)).await
    }

    /// DELETE a resource (ignores empty response body).
    pub async fn delete(&self, path: &str) -> CliResult<()> {
        let url = self.url(path);
        let mut attempt = 0usize;
        loop {
            let mut req = self.http.delete(&url);
            if let Some(t) = &self.token {
                req = req.bearer_auth(t);
            }
            match send_no_body(req).await {
                Ok(()) => return Ok(()),
                Err(ref e) if Self::is_retryable(e) && attempt < RETRY_DELAYS.len() => {
                    tokio::time::sleep(RETRY_DELAYS[attempt]).await;
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    async fn retry_json<T: DeserializeOwned>(
        &self,
        method: Method,
        path: &str,
        body: Option<&serde_json::Value>,
    ) -> CliResult<T> {
        let url = self.url(path);
        let mut attempt = 0usize;
        loop {
            let mut req = self.http.request(method.clone(), &url);
            if let Some(t) = &self.token {
                req = req.bearer_auth(t);
            }
            if let Some(b) = body {
                req = req.json(b);
            }
            match send_and_parse::<T>(req).await {
                Ok(v) => return Ok(v),
                Err(ref e) if Self::is_retryable(e) && attempt < RETRY_DELAYS.len() => {
                    tokio::time::sleep(RETRY_DELAYS[attempt]).await;
                    attempt += 1;
                }
                Err(e) => return Err(e),
            }
        }
    }

    fn is_retryable(err: &CliError) -> bool {
        match err {
            CliError::ServiceUnavailable | CliError::Connection(_) => true,
            CliError::Api { status, .. } if *status >= 500 => true,
            _ => false,
        }
    }
}

async fn send_and_parse<T: DeserializeOwned>(req: reqwest::RequestBuilder) -> CliResult<T> {
    let resp = req.send().await.map_err(map_reqwest_err)?;
    handle_response::<T>(resp).await
}

async fn send_no_body(req: reqwest::RequestBuilder) -> CliResult<()> {
    let resp = req.send().await.map_err(map_reqwest_err)?;
    let status = resp.status();
    if status.is_success() {
        return Ok(());
    }
    Err(extract_error(status, resp).await)
}

async fn handle_response<T: DeserializeOwned>(resp: Response) -> CliResult<T> {
    let status = resp.status();
    if status.is_success() {
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| CliError::Connection(e.to_string()))?;
        if bytes.is_empty() {
            return serde_json::from_str("null").map_err(CliError::Json);
        }
        return serde_json::from_slice(&bytes).map_err(CliError::Json);
    }
    Err(extract_error(status, resp).await)
}

async fn extract_error(status: StatusCode, resp: Response) -> CliError {
    let retry_after = resp
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(60);

    let body: ApiErrorBody = resp.json().await.unwrap_or_default();
    let msg = body.detail.or(body.message).unwrap_or(body.error);
    let msg = if msg.is_empty() {
        status.canonical_reason().unwrap_or("unknown").to_string()
    } else {
        msg
    };

    match status {
        StatusCode::UNAUTHORIZED => {
            CliError::Unauthorized("run `clawdb session create` first".to_string())
        }
        StatusCode::FORBIDDEN => CliError::PermissionDenied(msg),
        StatusCode::NOT_FOUND => CliError::NotFound(msg),
        StatusCode::TOO_MANY_REQUESTS => CliError::RateLimited {
            retry_after_secs: retry_after,
        },
        StatusCode::SERVICE_UNAVAILABLE => CliError::ServiceUnavailable,
        s => CliError::Api {
            status: s.as_u16(),
            message: msg,
        },
    }
}

fn map_reqwest_err(e: reqwest::Error) -> CliError {
    if e.is_connect() || e.is_timeout() {
        CliError::Connection("is clawdb-server running?".to_string())
    } else {
        CliError::Connection(e.to_string())
    }
}
