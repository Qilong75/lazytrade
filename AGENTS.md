# AGENTS.md

Guidance for coding agents working in this repository.

## Project Shape

- `src/` contains the Rust TUI application.
  - `main.rs` wires terminal setup, event handling, and the API polling loop.
  - `app.rs` owns application state, stock/group navigation, and stock list mutations.
  - `config.rs` loads and saves user config under the platform config directory.
  - `ui.rs`, `event.rs`, and `api.rs` handle rendering, terminal input/ticks, and market data.

## Common Commands

- Format Rust code: `cargo fmt`
- Check Rust code: `cargo check`
- Run tests when present: `cargo test`
- Run the TUI locally: `cargo run`

## macOS Packaging

- When updating `dist/lazytrade-macos`, always rebuild with `cargo build --release` and copy `target/release/lazytrade` into the intended `dist/` executable.
- After copying a macOS executable into `dist/`, always run `codesign --force --sign - <dist-binary>`; unsigned or stale-signature binaries can be killed by macOS at launch.
- After signing, refresh `dist/lazytrade-macos.tar.gz` from the signed `dist/lazytrade-macos` binary so archives do not reintroduce an old executable.
- Verify packaging changes with `codesign --verify --verbose=4 <dist-binary>` and `cargo test`. If practical, also launch the signed binary from a real terminal because Codex sandbox terminals may not support full TUI startup.

## Coding Standards

- Keep changes narrow and consistent with the existing module boundaries.
- Maintain idiomatic Rust style and run `cargo fmt` after Rust edits.
- New or changed function signatures must have a short comment explaining purpose, inputs, or side effects. Prefer Rust doc comments (`///`) for public functions and concise inline comments for private helpers when documentation would otherwise be noisy.
- Preserve terminal cleanup behavior around raw mode and alternate-screen handling.
- Avoid broad refactors while fixing a focused bug or adding a small feature.
- Do not silently swallow important failures. Existing best-effort persistence may stay best-effort, but new fallible behavior should either return an error or update user-visible status.

## TUI Product Direction

- The market-watching experience should stay as close as practical to Longbridge TUI: dense quote panels, watchlists, stock detail panes, intraday charts, candlestick/K-line views, volume panes, and source-backed market data.
- Interaction style should feel closer to lazygit: keyboard-first, fast navigation, clear focus states, predictable pane switching, compact status/help bars, and minimal modal friction.
- Do not copy Longbridge trading/account/OAuth behavior into this project. Use it only as a reference for行情展示、K线/分时图、布局密度、状态反馈, and TUI polish.
- Prefer improving the existing terminal workflow over adding marketing-style screens or broad visual redesigns.

## Data And Finance Workflow Rules

- Do not invent market data, financial metrics, dates, or source claims.
- Keep market data behavior source-backed and auditable in code comments, tests, or docs when changing providers.

## Git And Workspace Safety

- Assume the worktree may contain user changes. Do not revert unrelated edits.
- Do not run destructive commands such as `git reset --hard`, broad `rm`, or checkout-based rollback unless the user explicitly asks.
- Before committing, inspect the diff and stage only the intended files.
