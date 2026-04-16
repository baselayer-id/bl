---
name: hook-integrations
description: How `bl`'s IDE hook integrations work — the `bl startup` / `bl setup <target>` pattern, the per-target output contract (plain-text for Claude Code, JSON for Gemini CLI), and the recipe for adding a new IDE/agent target (Codex, Cursor, etc.). Use this skill when touching `src/commands/startup.rs`, `src/commands/setup.rs`, or adding a new `bl setup <target>` subcommand.
---

# Hook integrations

`bl` ships context into IDE/agent sessions by installing a SessionStart hook that
invokes `bl startup`. The primer payload (`GET /api/context/primer`) is the same
across targets — only the output **wrapping** differs.

## Two pieces per target

Every IDE target needs two cooperating pieces:

1. **Output wrapper** in `src/commands/startup.rs`
   - Matches the host's stdin/stdout contract
   - Selected via `--format <target>` on the `startup` command
2. **Installer** in `src/commands/setup.rs`
   - Writes the host's `settings.json` (or equivalent) with the right matcher
   - Must preserve unrelated keys and third-party hooks on install and remove

The two must be kept in sync — the installer's `command` field calls `bl startup`
with the `--format` flag that the wrapper knows how to emit.

## Current targets

| Target       | Host settings file         | Output contract                                           | Startup command                |
| ------------ | -------------------------- | --------------------------------------------------------- | ------------------------------ |
| Claude Code  | `~/.claude/settings.json`  | Raw text on stdout                                        | `bl startup`                   |
| Gemini CLI   | `~/.gemini/settings.json`  | JSON: `{"hookSpecificOutput":{"additionalContext":"…"}}`  | `bl startup --format gemini`   |

### Claude Code specifics

- Hooks: `SessionStart` **and** `PreCompact` (both fire the same `bl startup`)
- Matcher: `""` (empty — fires for everything)
- No output on failure — Claude Code treats empty stdout as a no-op

### Gemini CLI specifics

- Hook: `SessionStart` only
- Matcher: `startup|resume|clear` — fires on fresh start, `/resume`, and `/clear`
- Requires JSON on stdout. **Plain text will cause Gemini to error.**
- On failure (auth missing, API unreachable) we emit `{}` — Gemini accepts this
  as a valid no-op, whereas empty output would error
- Requires Gemini CLI v0.26.0+ (hooks on by default as of that release)

## The silent-on-error rule

Hook commands must **never block, prompt, or print a visible error**. If auth is
missing or the API is unreachable, `bl startup` exits `0` with an empty payload
appropriate for the target's format. Use `auth::get_bearer_token_silent()` — not
`get_bearer_token()` — for anything on the hook code path.

Why: a failing hook shouldn't break the user's IDE session. The worst case is
"no primer this session," not "IDE refuses to start."

## Adding a new target

To add, e.g., `bl setup codex` for the Codex CLI:

1. **Add a wrapper variant** in `src/commands/startup.rs`:
   - Extend the `Format` enum with your new variant
   - Extend `emit()` and `emit_empty()` with the host's output shape
2. **Wire it through clap** in `src/main.rs`:
   - Add the variant to the `StartupFormat` enum
   - Map it in the dispatcher arm that calls `commands::startup::run`
3. **Add the installer** in `src/commands/setup.rs`:
   - New `pub async fn codex(remove: bool) -> Result<()>` function
   - `install_codex_hooks()` / `remove_codex_hooks()` helpers
   - Mirror the Gemini helpers: read, mutate, write — never clobber unrelated keys
4. **Register the subcommand** in `src/main.rs`:
   - Add a `Codex { remove: bool }` variant to `SetupCommands`
   - Add the dispatcher arm
5. **Extend `bl setup check`** in `setup.rs`:
   - Report install status; use `-` (neutral) rather than `✗` when the host
     isn't installed at all — we don't want users to see red marks for IDEs
     they don't use
6. **Update `README.md`** — add a row to the commands table and a short section
   under "Hooks" describing the matcher and JSON shape
7. **Test** end-to-end with `HOME=/tmp/foo bl setup <target>` to verify the
   installer preserves unrelated settings and that remove doesn't clobber them

## Settings-file editing rules

Every installer follows the same pattern:

```rust
// 1. Read existing settings (or start with {})
// 2. Get or create the `hooks` object
// 3. INSERT or REPLACE the specific hook key (e.g. "SessionStart")
//    — do NOT clear the whole `hooks` object
// 4. Serialize pretty-printed JSON, write atomically
```

Remove follows the inverse pattern:

```rust
// 1. Read settings; bail if missing
// 2. Walk the target hook array, filter out entries whose
//    `/hooks/0/command` starts with "bl "
// 3. If the filtered array is empty, remove the hook key
// 4. If `hooks` is now empty, remove the `hooks` object entirely
// 5. Preserve every other top-level key (theme, auth config, etc.)
```

The "starts with `bl `" check is deliberate — it catches `bl startup`,
`bl startup --format gemini`, or any future `bl`-prefixed hook, without
touching third-party hooks the user might have added.

## Output format decision: `--format` vs separate commands

We chose `bl startup --format <target>` over `bl startup-gemini` (or equivalent)
because:

- The payload is identical across targets — only the wrapper differs
- `--format` keeps the wrapper logic in one file
- Clap auto-generates help text listing valid formats
- Easy to add new targets without proliferating top-level commands

If a future target needs genuinely different content (not just wrapping), revisit.

## Testing

Before committing a new target:

```bash
# Build
cargo build

# Verify JSON shape (if applicable)
./target/debug/bl startup --format <target> | python3 -m json.tool

# Isolated install/remove test — never touch the real ~/.xyz during dev
rm -rf /tmp/bl-test && mkdir -p /tmp/bl-test
HOME=/tmp/bl-test ./target/debug/bl setup <target>
cat /tmp/bl-test/.<target>/settings.json
HOME=/tmp/bl-test ./target/debug/bl setup <target> --remove
cat /tmp/bl-test/.<target>/settings.json

# Preservation test — pre-populate settings with theme/unrelated hooks,
# then install, then remove, and verify everything else is untouched
```

Also run `cargo clippy -- -D warnings` before cutting a release — CI is strict.
