# Repository Guidelines

## Project Structure & Module Organization
- `src/` holds the Rust code. Key modules: `main.rs` (CLI entry), `lib.rs` (shared types), `app.rs` (state/event handling), `ui.rs` (ratatui rendering), `scanner.rs` (disk scan logic), `cleaner.rs` (deletion/safety rules).
- `doc/` contains docs: `doc/usage.md` and `doc/architecture.md`.
- `Cargo.toml` and `Cargo.lock` define dependencies; `rustfmt.toml` sets formatting rules.
- `deny.toml` and `typos.toml` are configuration for optional tooling.
- `target/` is build output and should not be edited manually.

## Build, Test, and Development Commands
- `cargo build --release` builds the optimized binary.
- `./target/release/vac` runs the release build.
- `cargo run` builds and runs a dev build.
- `cargo nextest run` runs unit tests in `src/*`（基于 cargo-nextest v0.9.122 测试运行器）.
- `cargo nextest run -E 'test(test_name)'` runs a specific test by name.
- `cargo nextest list` lists all available tests without running them.
- `cargo fmt --all` formats code using `rustfmt.toml`.

### cargo-nextest 安装

macOS 预构建二进制安装（推荐）:

- `curl -LsSf https://get.nexte.st/latest/mac | tar zxf - -C ${CARGO_HOME:-~/.cargo}/bin`

或通过 Homebrew:

- `brew install cargo-nextest`

或通过源码安装:

- `cargo install --locked cargo-nextest`

### nextest 配置

项目级配置文件位于 `.config/nextest.toml`，支持多 profile 机制:

- 默认使用 `default` profile 运行测试。
- CI 环境推荐使用 `--profile ci` 以禁用 fail-fast 行为。
- 单个配置项可通过命令行参数、环境变量或配置文件覆盖。

## Coding Style & Naming Conventions
- Rust 2024 edition, 4-space indentation, and `max_width = 100` (see `rustfmt.toml`).
- Naming: `snake_case` for functions/vars, `PascalCase` for types, `SCREAMING_SNAKE_CASE` for constants.
- Prefer small, focused functions in `scanner` and `cleaner` with explicit error handling.

## Testing Guidelines
- Tests live alongside code in `#[cfg(test)]` modules (see `src/cleaner.rs`, `src/app.rs`, `src/scanner.rs`).
- Name tests by behavior in `snake_case`, e.g. `is_safe_to_delete_rejects_forbidden_paths`.
- Use temp directories (`tempfile`) for deletion and scanning scenarios to avoid touching real data.

## Commit & Pull Request Guidelines
- Recent history uses a `<type>:` prefix and emoji (e.g., `docs: 📝 ...`, `init: 🌱 ...`). Follow this style when reasonable.
- Keep commit subjects short and descriptive.
- PRs should describe changes, note any macOS-specific behavior, and include UI screenshots or terminal captures for TUI changes. List manual test steps (scan, select, clean) when applicable.

## Safety & Configuration Notes
- This tool deletes files; do not widen safe-delete rules without documenting rationale and tests.
- If running optional checks, `cargo deny check` and `typos` use `deny.toml` and `typos.toml` respectively.
