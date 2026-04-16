//! `bl search "query"` — semantic search across the vault.

use crate::client::Client;
use anyhow::Result;

pub async fn run(api_url: &str, query: &str) -> Result<()> {
    let client = Client::new(api_url)?;
    let response = client
        .mcp_call("memory_search", serde_json::json!({ "query": query }))
        .await?;

    println!("{response}");
    Ok(())
}
