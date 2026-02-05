# VAC (Vacuum)

VAC is a macOS disk-cleanup CLI tool built with `ratatui`. It provides a TUI (terminal UI) for scanning and cleaning common space-reclaimable directories.

## Features

- Fast scan of common macOS cleanup locations
- Directory browsing with size-based sorting
- Bulk selection with a confirmation step
- Scan progress with directory-size backfill
- Safety checks before deletion

## Requirements

- macOS
- Rust 1.85+ (`edition = 2024`)

## Quick Start

```bash
# Build
cargo build --release

# Run
./target/release/vac
```

## Key Bindings

- `s`: scan preset cleanup directories
- `S`: scan the user home directory
- `d`: scan a custom path
- `o`: toggle sort order (name/size)
- `Space`: select/unselect
- `a`: select/unselect all (current view)
- `c`: clean (enter confirmation)
- `?`: show/hide help
- `q`: quit

See `doc/usage.md` for full details.

## Docs

- Usage: `doc/usage.md`
- Architecture: `doc/architecture.md`

## Safety Notes

- Cleaning is irreversible; confirm carefully.
- Cleanup scope is limited by default to avoid deleting critical system directories.
- Back up important data before cleaning.

## License

MIT
