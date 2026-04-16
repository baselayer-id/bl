//! `bl setup` — install/remove IDE hooks.

use anyhow::{Context, Result};

/// Install or remove Claude Code hooks.
pub async fn claude(remove: bool) -> Result<()> {
    let home = dirs::home_dir().context("No home directory")?;

    if remove {
        remove_claude_hooks(&home)?;
        println!("✓ Removed Baselayer hooks from Claude Code");
    } else {
        install_claude_hooks(&home)?;
        println!("✓ Installed Baselayer hooks for Claude Code");
        println!();
        println!("  SessionStart → bl startup (context injection)");
        println!("  PreCompact   → bl startup (context preservation)");
        println!();
        println!("  Restart Claude Code for changes to take effect.");
    }
    Ok(())
}

/// Check all IDE integration status.
pub async fn check() -> Result<()> {
    let home = dirs::home_dir().context("No home directory")?;

    // Check bl binary
    let bl_path = which_bl();
    match &bl_path {
        Some(path) => println!("✓ bl CLI: {path}"),
        None => println!("✗ bl CLI: not in PATH"),
    }

    // Check Claude Code hooks
    let settings_path = home.join(".claude").join("settings.json");
    if settings_path.exists() {
        let content = std::fs::read_to_string(&settings_path).unwrap_or_default();
        if content.contains("bl startup") {
            println!("✓ Claude Code hooks: installed");
        } else {
            println!("✗ Claude Code hooks: not installed (run `bl setup claude`)");
        }
    } else {
        println!("✗ Claude Code hooks: no settings.json found");
    }

    // Check MCP config
    let claude_json = home.join(".claude.json");
    if claude_json.exists() {
        let content = std::fs::read_to_string(&claude_json).unwrap_or_default();
        if content.contains("baselayer") {
            println!("✓ Claude Code MCP: configured");
        } else {
            println!("✗ Claude Code MCP: not configured");
        }
    } else {
        println!("✗ Claude Code MCP: no config found");
    }

    // Check auth
    match crate::auth::get_display_key("https://api.baselayer.id") {
        Ok(Some(display)) => println!("✓ Authentication: {display}"),
        Ok(None) => println!("✗ Authentication: not signed in (run `bl auth login`)"),
        Err(_) => println!("✗ Authentication: keychain error"),
    }

    Ok(())
}

fn which_bl() -> Option<String> {
    std::process::Command::new("which")
        .arg("bl")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

fn install_claude_hooks(home: &std::path::Path) -> Result<()> {
    let settings_dir = home.join(".claude");
    std::fs::create_dir_all(&settings_dir).context("Failed to create ~/.claude/")?;

    let settings_path = settings_dir.join("settings.json");

    let mut config: serde_json::Value = if settings_path.exists() {
        let content =
            std::fs::read_to_string(&settings_path).context("Failed to read settings.json")?;
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let hooks = config
        .as_object_mut()
        .context("settings.json root is not an object")?
        .entry("hooks")
        .or_insert_with(|| serde_json::json!({}))
        .as_object_mut()
        .context("hooks is not an object")?;

    let hook_entry = serde_json::json!([{
        "matcher": "",
        "hooks": [{
            "type": "command",
            "command": "bl startup"
        }]
    }]);

    hooks.insert("SessionStart".to_string(), hook_entry.clone());
    hooks.insert("PreCompact".to_string(), hook_entry);

    let output = serde_json::to_string_pretty(&config)?;
    std::fs::write(&settings_path, output).context("Failed to write settings.json")?;

    Ok(())
}

fn remove_claude_hooks(home: &std::path::Path) -> Result<()> {
    let settings_path = home.join(".claude").join("settings.json");
    if !settings_path.exists() {
        return Ok(());
    }

    let content =
        std::fs::read_to_string(&settings_path).context("Failed to read settings.json")?;
    let mut config: serde_json::Value =
        serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}));

    if let Some(hooks) = config.get_mut("hooks").and_then(|h| h.as_object_mut()) {
        // Remove hooks that reference `bl startup`
        for key in ["SessionStart", "PreCompact"] {
            if let Some(entries) = hooks.get(key).and_then(|v| v.as_array()) {
                let filtered: Vec<_> = entries
                    .iter()
                    .filter(|entry| {
                        let has_bl = entry
                            .pointer("/hooks/0/command")
                            .and_then(|c| c.as_str())
                            .is_some_and(|c| c.starts_with("bl "));
                        !has_bl
                    })
                    .cloned()
                    .collect();

                if filtered.is_empty() {
                    hooks.remove(key);
                } else {
                    hooks.insert(key.to_string(), serde_json::Value::Array(filtered));
                }
            }
        }

        // Remove empty hooks object
        if hooks.is_empty() {
            config.as_object_mut().unwrap().remove("hooks");
        }
    }

    let output = serde_json::to_string_pretty(&config)?;
    std::fs::write(&settings_path, output).context("Failed to write settings.json")?;

    Ok(())
}
