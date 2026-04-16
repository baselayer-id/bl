//! `bl remember "text"` — record a memory for future recall.

use crate::client::Client;
use anyhow::Result;

pub async fn run(api_url: &str, text: &str, attach_to: Option<&str>) -> Result<()> {
    let client = Client::new(api_url)?;

    let mut args = serde_json::json!({ "content": text });
    if let Some(entity) = attach_to {
        args["attach_to_entity"] = serde_json::Value::String(entity.to_string());
    }

    let response = client.mcp_call("record_memory", args).await?;
    println!("{response}");
    Ok(())
}
