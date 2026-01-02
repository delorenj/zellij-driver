# zellij-driver

Redis-backed pane-first navigation manager for Zellij terminal sessions.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Overview

`zellij-driver` (CLI: `znav`) is a low-level manager for Zellij sessions that treats **panes as the primary unit of navigation**. Unlike traditional terminal multiplexers that focus on tabs, `znav` lets you navigate directly to named panes with automatic tab switching and focus.

Persistent state in Redis means your pane workspace survives Zellij restarts and provides deterministic navigation even when Zellij's native APIs lack stable IDs.

## Features

- **Pane-first navigation**: `znav pane <name>` jumps directly to a pane by name, creating it if missing
- **Auto-focus**: Automatically switches to the correct tab *and* pane position
- **Persistent state**: Redis-backed metadata survives Zellij restarts
- **Generic metadata**: Attach custom key-value metadata to panes for upstream tooling
- **Session-aware**: Cross-session navigation and creation
- **Idempotent**: Safe to run repeatedly; creates only if missing
- **Reconciliation**: Sync Redis state with actual Zellij layout

## Installation

### Prerequisites

- Zellij (latest release recommended)
- Redis server running locally
- Rust toolchain (for building from source)

### Build from source

```bash
git clone https://github.com/delorenj/zellij-driver.git
cd zellij-driver
cargo build --release
```

The binary will be at `target/release/znav`. Add to your `$PATH` or create a symlink:

```bash
ln -s $(pwd)/target/release/znav ~/.local/bin/znav
```

### Configuration

Create `/home/delorenj/.config/zellij-driver/config.toml`:

```toml
redis_url = "redis://127.0.0.1:6379"
```

Or set the Redis URL via environment:

```bash
export ZELLIJ_DRIVER_REDIS_URL="redis://127.0.0.1:6379"
```

## Usage

### Basic Navigation

```bash
# Create or focus a pane named "build"
znav pane build

# Create pane in specific tab
znav pane logs --tab monitoring

# Attach metadata
znav pane api-server --tab backend --meta project=myapp --meta env=dev
```

### Pane Information

```bash
# Get pane metadata as JSON
znav pane info build

# Example output:
# {
#   "pane_name": "build",
#   "session": "work",
#   "tab": "ci",
#   "created_at": "2025-01-02T10:15:42Z",
#   "last_accessed": "2025-01-02T10:18:09Z",
#   "meta": {
#     "project": "myapp"
#   },
#   "status": "found"
# }
```

### Tab Management

```bash
# Create or switch to a tab
znav tab backend

# List all tracked panes
znav list
```

### Reconciliation

```bash
# Sync Redis state with actual Zellij layout
znav reconcile
```

## How It Works

### Pane Creation

When you create a pane with `znav pane build --tab ci`:

1. Counts existing panes in "ci" tab
2. Creates new pane via `zellij action new-pane`
3. Renames pane to "build"
4. Stores metadata in Redis: `znav:pane:build` with fields:
   - `session`, `tab`, `position`, `created_at`, `last_seen`, `last_accessed`
   - Custom metadata from `--meta` flags

### Pane Navigation

When you navigate with `znav pane build`:

1. Looks up pane in Redis
2. Switches to pane's session (if needed)
3. Switches to pane's tab via `zellij action go-to-tab-name`
4. Focuses specific pane using stored position index
5. Updates `last_accessed` timestamp

### Auto-Focus Strategy

Panes are focused using sequential `focus-next-pane` commands based on stored position metadata. Position is captured at creation time by counting existing panes in the target tab.

## Redis Data Model

### Pane Hash

Key: `znav:pane:<pane_name>`

Fields:
- `session` - Zellij session name
- `tab` - Tab name
- `position` - Pane index in tab (for auto-focus)
- `created_at` - ISO8601 timestamp
- `last_seen` - Last reconciliation timestamp
- `last_accessed` - Last navigation timestamp
- `meta:*` - Generic user metadata fields

## Architecture

- **CLI Frontend** (`src/cli.rs`) - Argument parsing and command routing
- **ZellijDriver** (`src/zellij.rs`) - Executes `zellij action` commands
- **StateManager** (`src/state.rs`) - Redis CRUD operations
- **Orchestrator** (`src/orchestrator.rs`) - Business logic for pane-first navigation

## Limitations

- Pane focus uses sequential navigation; may fail if layout changes between sessions
- Position tracking unreliable for `CURRENT_TAB` panes (falls back to tab-only navigation)
- Requires `dump-layout --json` support for reconciliation (graceful fallback if unavailable)

## Contributing

Contributions welcome! This is a low-level primitive tool; keep it simple and focused.

### Development

```bash
# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy
```

## License

MIT License - see LICENSE file for details

## Acknowledgments

Built for developers who need stable, scriptable Zellij navigation with persistent state.
