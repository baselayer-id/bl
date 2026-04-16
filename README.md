# bl — Baselayer CLI

Terminal interface to your Baselayer knowledge vault. Ask questions, search
memories, record decisions, and inject session context into AI coding tools.

## Install

**Homebrew (recommended):**

```bash
brew install baselayer-id/tap/bl
```

**curl:**

```bash
curl -fsSL https://raw.githubusercontent.com/baselayer-id/bl/main/install.sh | bash
```

**From source:**

```bash
git clone https://github.com/baselayer-id/bl
cd bl
cargo build --release
cp target/release/bl ~/.local/bin/
```

## Quick start

```bash
bl auth login        # Sign in via browser (one-time)
bl setup claude      # Install Claude Code hooks
bl ask "what am I working on?"
```

## Commands

| Command              | What it does                                                   |
| -------------------- | -------------------------------------------------------------- |
| `bl auth login`      | Sign in via browser OAuth, store permanent API key in Keychain |
| `bl auth status`     | Show auth state, key display value, API connectivity           |
| `bl auth logout`     | Clear stored credentials                                       |
| `bl startup`         | Output compact session primer (designed for IDE hooks)         |
| `bl ask "question"`  | Ask your vault, get a synthesized answer                       |
| `bl search "query"`  | Semantic search across entities and facts                      |
| `bl remember "text"` | Record a memory (async distilled into graph)                   |
| `bl setup claude`    | Install Claude Code SessionStart/PreCompact hooks              |
| `bl setup gemini`    | Install Gemini CLI SessionStart hook                           |
| `bl setup check`     | Verify all integrations                                        |

## How auth works

`bl auth login` opens a browser, completes OAuth, and stores a permanent
`bl_*` API key in the macOS Keychain under `com.baselayer.cli`. This key
never expires until you revoke it.

You only need to `bl auth login` once per machine. The CLI does not share
credentials with the desktop app — each tool has its own key.

Revoke keys at https://app.baselayer.id/settings/api-keys.

## Hooks

### Claude Code

`bl setup claude` writes hooks to `~/.claude/settings.json` that call
`bl startup` on SessionStart and PreCompact events. The hook outputs a
compact context primer that Claude Code injects into the conversation.

### Gemini CLI

`bl setup gemini` writes a SessionStart hook to `~/.gemini/settings.json`
that calls `bl startup --format gemini`. Gemini requires a JSON response,
so the primer is wrapped as:

```json
{ "hookSpecificOutput": { "additionalContext": "…" } }
```

The matcher is `startup|resume|clear` — the primer is injected on fresh
startup, resumed sessions, and after `/clear`. Requires Gemini CLI
v0.26.0+ (hooks enabled by default).

### Primer contents

The primer is designed to be small (~1500 tokens) and includes:

- Your name and recent conversations
- Active plans
- Knowledge graph stats

Hooks exit silently if you're not signed in, so they never block session start.

## Development

```bash
# Build
cargo build --release -p bl

# Test against local stack
bl --api-url http://localhost:8080 auth login
bl --api-url http://localhost:8080 ask "test"

# Or via env var
BASELAYER_API_URL=http://localhost:8080 bl ask "test"
```

## Release

Update `Cargo.toml` version, then tag and push:

```bash
git tag v0.1.1
git push origin v0.1.1
```

This builds universal macOS binaries, creates a GitHub release, and
triggers the Homebrew tap to auto-update.

## License

MIT

