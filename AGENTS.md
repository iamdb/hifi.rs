# AGENTS.md

## Build / run

The task runner is [`just`](justfile) (`just --list`). Ignore the `Makefile.toml` / `hifirs/Makfile.toml` cargo-make config — it's stale and references a nonexistent `qobuz-client` crate.

- `just build-player` — full player build: installs native deps, builds the web UI, resets a local sqlite DB, then `cargo build --bin hifi-rs --release`.
- Building the `hifi-rs` binary has three prerequisites (all handled by `just build-player`):
  1. Native GStreamer 1.18+ dev libs (Debian: `libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev`). `cargo build` fails without them.
  2. `DATABASE_URL` must point at an existing, migrated sqlite DB. sqlx `query!`/`query_as!` macros are verified against a live DB at compile time; no `.sqlx` offline cache is committed.
  3. `www/build` must exist — it's embedded via `include_dir!("$CARGO_MANIFEST_DIR/../www/build")` (`hifirs/src/websocket.rs:18`). Build it with `cd www && npm install && npm run build`.

## Commands / testing

- The only Rust tests are in `qobuz-api` (insta snapshots). They hit the live Qobuz API (`can_use_methods`, `qobuz-api/src/client/api.rs:730`) and require paid-account env vars `QOBUZ_USERNAME`/`QOBUZ_PASSWORD` (see `.env.sample`); they fail without real credentials.
- Web UI lint/format: `cd www && npm run lint` / `npm run format` (prettier). No Rust formatter/linter is wired up.

## Architecture

Cargo workspace: three crates plus `www` (excluded from the workspace).

- `hifirs` → binary `hifi-rs`: cursive TUI player, GStreamer playback, sqlx/sqlite persistence, MPRIS, embedded axum web server + WebSocket API. Entrypoint `src/main.rs` → `cli::run()`.
- `qobuz-api` → lib `hifirs_qobuz_api`: unofficial Qobuz API client (auth + search/album/track/playlist).
- `playlist-sync` → binary `hifirs-playlist-sync`: Spotify→Qobuz sync (rspotify; needs `RSPOTIFY_*` env).
- `www` → SvelteKit UI compiled to `www/build` and embedded in the binary.

Migrations live in `hifirs/migrations/` (sqlx `migrate!`, `build.rs` re-runs on change). New migrations need both `.up.sql` and `.down.sql`. The `config` table (single row, `ROWID = 1`) stores Qobuz auth state.

## Gotchas / conventions

- Log filter env var is `HIFIRS_LOG` (`hifirs/src/cli.rs:238`), not `RUST_LOG` despite what `.env.sample` says.
- `hifirs/src/sql/mod.rs` defines custom macros `query!`, `get_one!`, `get_all!`, `acquire!` wrapping sqlx — use these in db code instead of raw sqlx calls.
- Qobuz password is stored in the DB as an MD5 hash (`hifirs/src/cli.rs:409`).
- `mpris` is `#[cfg(target_os = "linux")]` only — don't reference it from portable code.
- `default.vim` is an auto-generated Vim session file, not configuration; ignore it.
