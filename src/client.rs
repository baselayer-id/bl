//! Baselayer API client — thin wrapper over reqwest for authenticated requests.

use anyhow::{Context, Result};
use serde_json::Value;

use crate::auth;

pub struct Client {
    api_url: String,
    token: String,
    http: reqwest::Client,
}

impl Client {
    /// Create a new authenticated client. Reads API key from Keychain.
    pub fn new(api_url: &str) -> Result<Self> {
        let token = auth::get_bearer_token(api_url)?;
        Self::with_token(api_url, &token)
    }

    /// Create a client with an explicit token (for hooks and login flow).
    pub fn with_token(api_url: &str, token: &str) -> Result<Self> {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent(concat!("bl/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            api_url: api_url.trim_end_matches('/').to_string(),
            token: token.to_string(),
            http,
        })
    }

    /// GET a plain-text endpoint.
    pub async fn get_text(&self, path: &str) -> Result<String> {
        let url = format!("{}{}", self.api_url, path);
        let resp = self
            .http
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "text/plain")
            .send()
            .await
            .with_context(|| {
                format!(
                    "Could not connect to Baselayer API at {url}\n\n\
                     Possible causes:\n\
                     - No internet connection\n\
                     - The Baselayer API is down\n\
                     - If using a local server: is `docker compose up` running?\n\n\
                     Check your connection and try again."
                )
            })?;

        self.handle_status(&resp).await?;
        resp.text().await.context("Failed to read response body")
    }

    /// Call an MCP tool via JSON-RPC and return the text content.
    pub async fn mcp_call(&self, tool: &str, args: Value) -> Result<String> {
        let url = format!("{}/mcp", self.api_url);
        let payload = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": tool,
                "arguments": args,
            }
        });

        let resp = self
            .http
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .with_context(|| {
                format!(
                    "Could not connect to Baselayer MCP endpoint at {url}\n\n\
                     Possible causes:\n\
                     - No internet connection\n\
                     - The Baselayer API is down\n\
                     - If using a local server: is `docker compose up` running?\n\n\
                     Check your connection and try again."
                )
            })?;

        self.handle_status(&resp).await?;

        let body: Value = resp
            .json()
            .await
            .context("Failed to parse MCP response as JSON")?;

        // Extract text content from MCP response
        if let Some(error) = body.get("error") {
            let msg = error
                .get("message")
                .and_then(|m| m.as_str())
                .unwrap_or("Unknown MCP error");
            let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(0);
            anyhow::bail!(
                "MCP error (code {code}): {msg}\n\n\
                 This is an error from the Baselayer MCP server. If it persists,\n\
                 check your Baselayer account status at https://app.baselayer.id"
            );
        }

        let content = body
            .pointer("/result/content")
            .and_then(|c| c.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                            item.get("text").and_then(|t| t.as_str()).map(String::from)
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        if content.is_empty() {
            anyhow::bail!(
                "MCP tool '{tool}' returned no content.\n\n\
                 This usually means your vault is empty. Try capturing some\n\
                 conversations first via the Baselayer desktop app or Chrome\n\
                 extension."
            );
        }

        Ok(content)
    }

    async fn handle_status(&self, resp: &reqwest::Response) -> Result<()> {
        match resp.status().as_u16() {
            200..=299 => Ok(()),
            401 => anyhow::bail!(
                "Authentication expired or invalid (HTTP 401).\n\n\
                 To fix this:\n\
                 1. Open the Baselayer desktop app\n\
                 2. If signed out, sign back in\n\
                 3. Retry your command\n\n\
                 The CLI uses the same auth tokens as the desktop app. If the\n\
                 desktop app is signed in but this error persists, try signing\n\
                 out and back in to refresh the token."
            ),
            403 => anyhow::bail!(
                "Access forbidden (HTTP 403).\n\n\
                 Your account may be suspended or lack permissions for this\n\
                 action. Check your account status at https://app.baselayer.id"
            ),
            426 => anyhow::bail!(
                "Client version too old (HTTP 426).\n\n\
                 To fix this:\n\
                 1. Update the CLI: `brew upgrade bl` or rebuild from source\n\
                 2. Update the desktop app if prompted\n\
                 3. Retry your command"
            ),
            429 => anyhow::bail!(
                "Rate limited (HTTP 429).\n\n\
                 You've made too many requests in a short period. Wait a\n\
                 moment and try again. If you're on the free tier, consider\n\
                 upgrading at https://app.baselayer.id/settings/billing"
            ),
            503 => anyhow::bail!(
                "Baselayer is undergoing maintenance (HTTP 503).\n\n\
                 The service is temporarily unavailable. Try again in a few\n\
                 minutes."
            ),
            status => anyhow::bail!(
                "Unexpected API error (HTTP {status}).\n\n\
                 If this persists, check https://status.baselayer.id or\n\
                 report the issue at https://github.com/baselayer-id/baselayer/issues"
            ),
        }
    }
}
