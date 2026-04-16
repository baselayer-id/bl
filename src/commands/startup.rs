//! `bl startup` — compact session primer for IDE hooks.
//!
//! Outputs a concise context block to stdout. Designed to be called by
//! Claude Code SessionStart/PreCompact hooks (plain text) or Gemini CLI
//! SessionStart hooks (JSON wrapper). Optimized for speed: exits silently
//! if not authenticated so it never blocks session start.

use crate::auth;
use crate::client::Client;
use anyhow::Result;

#[derive(Clone, Copy)]
pub enum Format {
    /// Plain text to stdout (Claude Code).
    Text,
    /// JSON with `hookSpecificOutput.additionalContext` (Gemini CLI).
    Gemini,
}

pub async fn run(api_url: &str, format: Format) -> Result<()> {
    // Use silent auth check — hooks must never block or prompt
    let token = match auth::get_bearer_token_silent(api_url) {
        Some(t) => t,
        None => return emit_empty(format),
    };

    let client = match Client::with_token(api_url, &token) {
        Ok(c) => c,
        Err(_) => return emit_empty(format),
    };

    match client.get_text("/api/context/primer").await {
        Ok(text) => emit(format, &text),
        Err(_) => emit_empty(format), // API unreachable — skip silently
    }
}

fn emit(format: Format, text: &str) -> Result<()> {
    match format {
        Format::Text => print!("{text}"),
        Format::Gemini => {
            let out = serde_json::json!({
                "hookSpecificOutput": {
                    "additionalContext": text,
                }
            });
            print!("{out}");
        }
    }
    Ok(())
}

/// Emit an empty payload appropriate for the format.
///
/// Claude Code's hook accepts no output (skips context injection).
/// Gemini CLI requires valid JSON on stdout or it errors — so we emit `{}`.
fn emit_empty(format: Format) -> Result<()> {
    if matches!(format, Format::Gemini) {
        print!("{{}}");
    }
    Ok(())
}
