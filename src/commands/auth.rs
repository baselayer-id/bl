//! `bl auth` — authentication management.
//!
//! Login flow:
//! 1. Start localhost callback server on random port
//! 2. Open browser to baselayer.id/login?desktop=true&callback_port=PORT
//! 3. User signs in via Google/GitHub
//! 4. Web app POSTs Firebase tokens to localhost callback
//! 5. CLI uses the Firebase ID token to call POST /keys/connect
//! 6. CLI stores the returned `bl_*` API key in macOS Keychain
//! 7. All subsequent commands use the permanent API key

use std::sync::Arc;

use anyhow::{Context, Result};
use axum::{extract::Json, response::Html, routing::post, Router};
use serde::Deserialize;
use tokio::sync::{oneshot, Mutex};
use tower_http::cors::{Any, CorsLayer};

use crate::auth;

#[derive(Debug, Deserialize)]
struct AuthCallback {
    #[serde(rename = "idToken")]
    id_token: String,
    #[allow(dead_code)]
    #[serde(rename = "refreshToken")]
    refresh_token: String,
    #[allow(dead_code)]
    uid: String,
    email: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ConnectKeyResponse {
    api_key: String,
    display_value: String,
    #[allow(dead_code)]
    name: String,
}

pub async fn login(api_url: &str) -> Result<()> {
    // Check if already logged in
    if let Some(display) = auth::get_display_key(api_url)? {
        println!("Already signed in ({display}).");
        println!();
        println!("To sign in as a different account, run `bl auth logout` first.");
        return Ok(());
    }

    println!("Opening browser to sign in...");

    // 1. Start localhost callback server
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.context(
        "Failed to bind to localhost for OAuth callback.\n\n\
             Ensure no firewall is blocking local connections on 127.0.0.1.",
    )?;

    let port = listener.local_addr()?.port();

    let (tx, rx) = oneshot::channel::<AuthCallback>();
    let tx = Arc::new(Mutex::new(Some(tx)));

    let tx_clone = tx.clone();
    let router = Router::new()
        .route(
            "/auth-callback",
            post(move |payload: Json<AuthCallback>| {
                let tx = tx_clone.clone();
                async move {
                    if let Some(sender) = tx.lock().await.take() {
                        let _ = sender.send(payload.0);
                        success_page()
                    } else {
                        error_page("Already processed")
                    }
                }
            }),
        )
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        );

    let server_handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, router).await {
            eprintln!("OAuth server error: {e}");
        }
    });

    // 2. Open browser
    let login_base = if api_url.contains("localhost") || api_url.contains("127.0.0.1") {
        "http://localhost:3000/login".to_string()
    } else if api_url.contains("api-dev") {
        "https://app-dev.baselayer.id/login".to_string()
    } else {
        "https://baselayer.id/login".to_string()
    };

    let login_url = format!("{login_base}?desktop=true&callback_port={port}");
    open::that(&login_url).context(
        "Failed to open your browser.\n\n\
         To sign in manually, open this URL:\n\
         {login_url}",
    )?;

    println!("Waiting for sign-in (2 minute timeout)...");

    // 3. Wait for callback
    let callback = tokio::time::timeout(std::time::Duration::from_secs(120), rx)
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "Sign-in timed out after 2 minutes.\n\n\
                 To fix this:\n\
                 1. Run `bl auth login` again\n\
                 2. Complete the sign-in in the browser window that opens\n\
                 3. The browser will redirect back automatically"
            )
        })?
        .map_err(|_| anyhow::anyhow!("Sign-in was cancelled"))?;

    server_handle.abort();

    let email_display = callback.email.as_deref().unwrap_or("unknown");
    println!("Signed in as {email_display}. Creating API key...");

    // 4. Use the Firebase token to create a permanent API key
    let hostname = gethostname();
    let key_name = format!("bl CLI on {hostname}");

    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{api_url}/api/keys/connect"))
        .header("Authorization", format!("Bearer {}", callback.id_token))
        .header("Content-Type", "application/json")
        .json(&serde_json::json!({ "name": key_name }))
        .send()
        .await
        .context(
            "Failed to connect to Baselayer API to create API key.\n\n\
             Possible causes:\n\
             - No internet connection\n\
             - The Baselayer API is unreachable\n\n\
             Your browser sign-in succeeded, but the CLI couldn't\n\
             create an API key. Run `bl auth login` to retry.",
        )?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!(
            "Failed to create API key (HTTP {status}): {body}\n\n\
             Your browser sign-in succeeded, but API key creation failed.\n\
             Run `bl auth login` to retry."
        );
    }

    let key_resp: ConnectKeyResponse = resp
        .json()
        .await
        .context("Failed to parse API key response")?;

    // 5. Store in Keychain
    auth::store_api_key(api_url, &key_resp.api_key)?;

    println!();
    println!("✓ Authenticated successfully!");
    println!("  Key: {}", key_resp.display_value);
    println!(
        "  Stored in: macOS Keychain ({})",
        auth::service_name(api_url)
    );
    println!();
    println!("  All `bl` commands will now authenticate automatically.");
    println!("  To set up Claude Code hooks, run: bl setup claude");

    Ok(())
}

pub async fn status(api_url: &str) -> Result<()> {
    match auth::get_display_key(api_url)? {
        Some(display) => {
            println!("✓ Authenticated");
            println!("  Key: {display}");
            println!("  Keychain: {}", auth::service_name(api_url));

            // Quick API health check
            match crate::client::Client::new(api_url) {
                Ok(client) => match client.get_text("/api/context/primer").await {
                    Ok(_) => println!("  API: connected"),
                    Err(e) => println!("  API: error — {e}"),
                },
                Err(e) => println!("  API: {e}"),
            }
        }
        None => {
            println!("✗ Not authenticated");
            println!();
            println!("  Run `bl auth login` to sign in.");
        }
    }
    Ok(())
}

pub async fn logout() -> Result<()> {
    let mut cleared = false;
    for url in [
        "https://api.baselayer.id",
        "https://api-dev.baselayer.id",
        "http://localhost:8080",
    ] {
        if auth::get_display_key(url)?.is_some() {
            auth::clear_tokens(url)?;
            cleared = true;
        }
    }

    if cleared {
        println!("✓ Signed out. API key removed from Keychain.");
        println!();
        println!("  Note: The API key has been removed locally but is still");
        println!("  active on the server. To revoke it, visit:");
        println!("  https://app.baselayer.id/settings/api-keys");
    } else {
        println!("No stored credentials found. Already signed out.");
    }
    Ok(())
}

fn gethostname() -> String {
    std::process::Command::new("hostname")
        .arg("-s")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|| "unknown".to_string())
}

fn success_page() -> Html<String> {
    Html(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Signed In — Baselayer CLI</title>
    <style>
        body {
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex; align-items: center; justify-content: center;
            height: 100vh; margin: 0;
            background: linear-gradient(135deg, #132744 0%, #2fa9a4 100%);
            color: white;
        }
        .container {
            text-align: center; padding: 3rem;
            background: rgba(255,255,255,0.1); border-radius: 1.5rem;
            backdrop-filter: blur(10px);
            box-shadow: 0 8px 32px rgba(0,0,0,0.3);
        }
        h1 { font-size: 2rem; margin: 1rem 0 0.5rem; }
        p { font-size: 1.1rem; opacity: 0.9; }
        .check { font-size: 3rem; }
    </style>
</head>
<body>
    <div class="container">
        <div class="check">✓</div>
        <h1>CLI Authenticated</h1>
        <p>You can close this tab and return to your terminal.</p>
    </div>
    <script>setTimeout(() => window.close(), 2000);</script>
</body>
</html>"#
            .to_string(),
    )
}

fn error_page(message: &str) -> Html<String> {
    Html(format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>Error — Baselayer CLI</title>
    <style>
        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
            display: flex; align-items: center; justify-content: center;
            height: 100vh; margin: 0;
            background: linear-gradient(135deg, #132744 0%, #e74c3c 100%);
            color: white;
        }}
        .container {{
            text-align: center; padding: 3rem;
            background: rgba(255,255,255,0.1); border-radius: 1.5rem;
            backdrop-filter: blur(10px);
        }}
        h1 {{ font-size: 2rem; }}
        p {{ font-size: 1.1rem; opacity: 0.9; }}
    </style>
</head>
<body>
    <div class="container">
        <h1>Error</h1>
        <p>{message}</p>
        <p style="opacity:0.6; font-size:0.9rem;">Close this window and try again.</p>
    </div>
</body>
</html>"#
    ))
}
