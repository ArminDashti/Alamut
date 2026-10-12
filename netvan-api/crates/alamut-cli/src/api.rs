//! Thin async JSON-RPC client for the Alamut API (`POST /api/rpc`).
//!
//! Reuses `alamut_core::ipc` request/response types so the wire format can
//! never drift from the service implementation.

use anyhow::{anyhow, Result};
use alamut_core::ipc::{RpcRequest, RpcResponse};

#[derive(Clone)]
pub struct ApiClient {
    base: String,
    http: reqwest::Client,
}

impl ApiClient {
    pub fn new(base: impl Into<String>) -> Self {
        let mut base = base.into();
        while base.ends_with('/') {
            base.pop();
        }
        Self {
            base,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
        }
    }

    /// Send one RPC request; turns a service-side `Error` into `Err`.
    pub async fn call(&self, req: RpcRequest) -> Result<RpcResponse> {
        let url = format!("{}/api/rpc", self.base);
        let resp = self
            .http
            .post(&url)
            .json(&req)
            .send()
            .await
            .map_err(|e| anyhow!("API unreachable at {} ({e})", self.base))?;
        let status = resp.status();
        if !status.is_success() {
            return Err(anyhow!("API HTTP {status}"));
        }
        let value: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| anyhow!("bad API response: {e}"))?;
        let parsed: RpcResponse =
            serde_json::from_value(value).map_err(|e| anyhow!("undecodable RPC reply: {e}"))?;
        match parsed {
            RpcResponse::Error { message } => Err(anyhow!(message)),
            other => Ok(other),
        }
    }

}

/// Decode a successful reply into a concrete payload variant.
macro_rules! grab {
    ($res:expr => $variant:ident, $errors:expr) => {
        match $res {
            Ok(RpcResponse::$variant(v)) => Some(v),
            Ok(other) => {
                $errors.push(format!("unexpected reply: {other:?}"));
                None
            }
            Err(e) => {
                $errors.push(e.to_string());
                None
            }
        }
    };
}

pub(crate) use grab;
