//! `bl startup` — compact session primer for IDE hooks.
//!
//! Outputs a concise context block to stdout. Designed to be called by
//! Claude Code SessionStart/PreCompact hooks. Optimized for speed:
//! exits silently if not authenticated so it never blocks session start.

use crate::auth;
use crate::client::Client;
use anyhow::Result;

pub async fn run(api_url: &str) -> Result<()> {
    // Use silent auth check — hooks must never block or prompt
    let token = match auth::get_bearer_token_silent(api_url) {
        Some(t) => t,
        None => return Ok(()), // Not logged in — skip silently
    };

    let client = match Client::with_token(api_url, &token) {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    match client.get_text("/api/context/primer").await {
        Ok(text) => {
            print!("{text}");
            Ok(())
        }
        Err(_) => Ok(()), // API unreachable — skip silently
    }
}
