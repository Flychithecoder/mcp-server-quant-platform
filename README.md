# Quant Platform MCP — Zed extension

Exposes the platform's MCP surface to Zed's agent panel: backtest, optimise,
Monte Carlo and rule-significance from Jesse core, plus this repo's
live-session tools.

## Status

| | |
|---|---|
| Type-checks against `zed_extension_api` 0.7.0 | yes (`cargo check --release`) |
| Built to `wasm32-wasip1` | **not yet** — see below |
| Installed as a Zed dev extension | **not yet** |
| Published to zed.dev/extensions | **blocked** — see below |

The wasm build was not run here because `static.rust-lang.org` is unreachable
from the build sandbox, so `rustup target add wasm32-wasip1` fails. That is an
environment limit, not a code problem — the extension type-checks against the
real API. Run this on a machine with normal network access before trusting it:

```bash
rustup target add wasm32-wasip1
cd integrations/zed
cargo build --release --target wasm32-wasip1
```

Then install it in Zed: `zed: install dev extension` from the command palette,
and point it at this directory.

## Why the server needed a new transport

Zed launches the MCP server itself and speaks to it over the child process's
stdin/stdout. Our server's default is a long-running HTTP listener on `:9003`,
which Zed would start and then wait on forever with nothing on the pipe. Hence
`--transport stdio`, added to `jesse_live.mcp.server` alongside the existing
HTTP path, which is unchanged and still the default.

Under stdio, **stdout belongs to the JSON-RPC stream**. A single log line
written there corrupts a frame and the client disconnects with a parse error
that names no cause, so logging is moved to stderr before the first log line.
There is a test asserting no handler is left pointing at stdout.

## Settings

```jsonc
// ~/.config/zed/settings.json
{
  "context_servers": {
    "quant-platform": {
      "source": "extension",
      "settings": {
        // The interpreter that has jesse_live installed.
        // NOT a bare "python3": on Windows that is the WindowsApps stub, and a
        // venv's Scripts/ has no python3.exe at all.
        "python_path": "/absolute/path/to/.venv/bin/python",
        "project_path": "/absolute/path/to/your/jesse/project",
        "api_url": "http://localhost:9000",
        "surface": "research"
      }
    }
  }
}
```

`surface` selects what the agent can reach:

| value | tools |
|---|---|
| `research` (default) | backtest, optimise, Monte Carlo, significance, candles, indicators |
| `live` | live-session tools only — no backtest surface |
| `both` | everything |

**It defaults to `research` deliberately.** An editor agent gets the read-only
surface unless someone widens it on purpose. `live` includes
`start_live_session`, and while that path is guarded (scope-bound arm token,
`_check_start` against `guard.json`, an audit record that must be writable
before a live start proceeds), the cheapest control is not handing it to an
agent that only needs to run backtests.

**Leave `password` unset.** Zed's `settings.json` is not a secret store and is
routinely pasted into bug reports. Unset, the server reads the project's `.env`.
If you must set it, the extension passes it via the environment and never in
argv — argv is visible in `ps` to every user on the machine.

## Relationship to quant-platform

This repo is **public and deliberately thin**: a manifest and ~200 lines of Rust
that return a command. It contains no strategy, no credential, no venue driver
and no licensing code. The platform it launches
(`Flychithecoder/quant-platform`) stays private.

That split exists because Zed's registry adds your repository to
[`zed-industries/extensions`](https://github.com/zed-industries/extensions) as a
**git submodule**, so the repo must be public. Publishing this one costs nothing;
publishing the platform would have meant publishing the live layer, the venue
drivers and the licensing system.

## Publishing to the registry

```bash
# 1. this repo must be public, tagged, and CI-green
# 2. fork zed-industries/extensions
git submodule add https://github.com/Flychithecoder/mcp-server-quant-platform \
    extensions/mcp-server-quant-platform
# 3. add the entry to the top-level extensions.toml, then
pnpm sort-extensions
# 4. open the PR
```

`scripts/check_manifest.py` asserts the rules Zed rejects on at review — required
fields, reserved words in the id (`zed`, `extension`), an approved licence, an
actual `[context_servers.*]` table, and that `src/lib.rs` implements
`context_server_command` rather than merely declaring one. It runs in CI, and it
has been mutation-tested: five deliberately broken manifests, five refusals.
