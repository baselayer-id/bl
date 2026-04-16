//! `bl ask "question"` — ask the vault and get a synthesized answer.

use crate::client::Client;
use anyhow::Result;

pub async fn run(api_url: &str, question: &str) -> Result<()> {
    let client = Client::new(api_url)?;
    let response = client
        .mcp_call("ask_question", serde_json::json!({ "question": question }))
        .await?;

    println!("{response}");
    Ok(())
}
