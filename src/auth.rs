//! Auth for the Baselayer CLI.
//!
//! The CLI uses its own permanent `bl_*` API key, stored in the macOS Keychain
//! under `com.baselayer.cli`. Created by `bl auth login` via browser OAuth.
//! Never expires until revoked.
//!
//! No fallback to the desktop app's Firebase tokens — one auth mechanism,
//! one source of truth.

use anyhow::{Context, Result};

// ── Keychain constants ──

const KEYCHAIN_NAME: &str = "baselayer.keychain-db";
const KEYCHAIN_PASSWORD: &str = "baselayer";
const CLI_KEY_NAME: &str = "cli-api-key";

/// Derive the Keychain service name from the API URL.
pub fn service_name(api_url: &str) -> String {
    if api_url.contains("api-dev") {
        "com.baselayer.cli.dev".to_string()
    } else if api_url.contains("localhost") || api_url.contains("127.0.0.1") {
        "com.baselayer.cli.local".to_string()
    } else {
        "com.baselayer.cli".to_string()
    }
}

fn keychain_path() -> String {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    format!("{}/Library/Keychains/{}", home, KEYCHAIN_NAME)
}

fn ensure_keychain() -> Result<()> {
    let path = keychain_path();
    if !std::path::Path::new(&path).exists() {
        let output = std::process::Command::new("security")
            .args(["create-keychain", "-p", KEYCHAIN_PASSWORD, &path])
            .output()
            .context("Failed to create keychain")?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if !stderr.contains("already exists") {
                anyhow::bail!(
                    "Failed to create Baselayer keychain at {path}: {}\n\n\
                     This keychain stores your authentication credentials securely.\n\
                     Ensure you have write access to ~/Library/Keychains/",
                    stderr.trim()
                );
            }
        }
    }

    // Unlock
    let _ = std::process::Command::new("security")
        .args(["unlock-keychain", "-p", KEYCHAIN_PASSWORD, &path])
        .output();
    // No auto-lock
    let _ = std::process::Command::new("security")
        .args(["set-keychain-settings", &path])
        .output();

    Ok(())
}

fn keychain_read(service: &str, account: &str) -> Result<Option<String>> {
    ensure_keychain()?;
    let path = keychain_path();

    let output = std::process::Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            service,
            "-a",
            account,
            "-w",
            &path,
        ])
        .output()
        .context("Failed to read from keychain")?;

    if !output.status.success() {
        return Ok(None);
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    Ok(Some(trimmed.to_string()))
}

pub fn keychain_write(service: &str, account: &str, value: &str) -> Result<()> {
    ensure_keychain()?;
    let path = keychain_path();

    // Delete existing entry first
    let _ = std::process::Command::new("security")
        .args([
            "delete-generic-password",
            "-s",
            service,
            "-a",
            account,
            &path,
        ])
        .output();

    let output = std::process::Command::new("security")
        .args([
            "add-generic-password",
            "-s",
            service,
            "-a",
            account,
            "-w",
            value,
            "-U",
            &path,
        ])
        .output()
        .context("Failed to write to keychain")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!(
            "Failed to store credentials in keychain: {}\n\n\
             Try closing Keychain Access.app if it's open, then retry.",
            stderr.trim()
        );
    }

    Ok(())
}

fn keychain_delete(service: &str, account: &str) -> Result<()> {
    ensure_keychain()?;
    let path = keychain_path();

    let output = std::process::Command::new("security")
        .args([
            "delete-generic-password",
            "-s",
            service,
            "-a",
            account,
            &path,
        ])
        .output()
        .context("Failed to delete from keychain")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("could not be found") {
            anyhow::bail!("Failed to clear credentials: {}", stderr.trim());
        }
    }

    Ok(())
}

// ── Public API ──

/// Get the stored `bl_*` API key, or error with instructions to log in.
pub fn get_bearer_token(api_url: &str) -> Result<String> {
    let service = service_name(api_url);
    match keychain_read(&service, CLI_KEY_NAME)? {
        Some(key) if key.starts_with("bl_") => Ok(key),
        _ => anyhow::bail!(
            "Not signed in to Baselayer.\n\n\
             To fix this, run:\n\
               bl auth login\n\n\
             This opens your browser, signs you in, and stores a permanent\n\
             API key in your macOS Keychain. You only need to do this once."
        ),
    }
}

/// Get the stored API key without erroring (for silent hook use).
pub fn get_bearer_token_silent(api_url: &str) -> Option<String> {
    let service = service_name(api_url);
    keychain_read(&service, CLI_KEY_NAME)
        .ok()
        .flatten()
        .filter(|k| k.starts_with("bl_"))
}

/// Store a `bl_*` API key in the Keychain.
pub fn store_api_key(api_url: &str, api_key: &str) -> Result<()> {
    let service = service_name(api_url);
    keychain_write(&service, CLI_KEY_NAME, api_key)
}

/// Get display-safe representation of the stored key.
pub fn get_display_key(api_url: &str) -> Result<Option<String>> {
    let service = service_name(api_url);
    match keychain_read(&service, CLI_KEY_NAME)? {
        Some(key) if key.starts_with("bl_") && key.len() > 19 => Ok(Some(format!(
            "bl_{}...{}",
            &key[3..11],
            &key[key.len() - 8..]
        ))),
        Some(key) if key.starts_with("bl_") => Ok(Some(key)),
        _ => Ok(None),
    }
}

/// Clear CLI credentials.
pub fn clear_tokens(api_url: &str) -> Result<()> {
    let service = service_name(api_url);
    keychain_delete(&service, CLI_KEY_NAME)
}
