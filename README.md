# Perth (zellij-driver)

Cognitive context manager for Zellij terminal sessions with intent tracking and Redis-backed persistence.

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

## Overview

Perth (CLI: `zdrive`) is a workspace context manager for Zellij that combines **pane-first navigation** with **intent tracking**. Track what you're working on, log milestones, and maintain context across sessions.

Key capabilities:
- **Intent Logging**: Record what you're working on with `zdrive pane log`
- **Context History**: Review your work history with `zdrive pane history`
- **Pane Navigation**: Jump directly to named panes with automatic tab switching
- **Persistent State**: Redis-backed metadata survives Zellij restarts

## Quick Start

```bash
# Navigate to or create a pane
zdrive pane my-feature

# Log what you're working on
zdrive pane log my-feature "Implementing user authentication"

# Mark a milestone
zdrive pane log my-feature "Completed OAuth integration" --type milestone

# Log with artifacts
zdrive pane log my-feature "Fixed login bug" --artifacts src/auth.rs tests/auth_test.rs

# View history
zdrive pane history my-feature

# View last 5 entries in JSON
zdrive pane history my-feature --last 5 --format json
```

## Installation

### Prerequisites

- Zellij v0.39.0 or later
- Redis server running locally
- Rust toolchain (for building from source)

### Build from source

```bash
git clone https://github.com/delorenj/zellij-driver.git
cd zellij-driver
cargo build --release
```

The binary will be at `target/release/zdrive`. Add to your `$PATH`:

```bash
ln -s $(pwd)/target/release/zdrive ~/.local/bin/zdrive
```

### Configuration

View current config:

```bash
zdrive config show
```

Set Redis URL:

```bash
zdrive config set redis_url redis://localhost:6379/
```

Or create `~/.config/zellij-driver/config.toml`:

```toml
redis_url = "redis://127.0.0.1:6379/"
```

## Intent Tracking

### Logging Work

Record your progress with typed entries:

```bash
# Regular checkpoint (default)
zdrive pane log api-work "Refactoring request handlers"

# Major milestone
zdrive pane log api-work "Released v2.0 API" --type milestone

# Research/exploration
zdrive pane log api-work "Investigating caching strategies" --type exploration

# With file artifacts
zdrive pane log api-work "Added rate limiting" --artifacts src/middleware/rate_limit.rs
```

### Viewing History

```bash
# Human-readable output with colors and relative timestamps
zdrive pane history my-feature

# Last N entries
zdrive pane history my-feature --last 10

# JSON output for tooling
zdrive pane history my-feature --format json

# Compact JSON for piping
zdrive pane history my-feature --format json-compact | jq '.entries[0]'
```

### Entry Types

| Type | Icon | Use For |
|------|------|---------|
| `checkpoint` | ● | Regular progress markers |
| `milestone` | ★ | Major accomplishments |
| `exploration` | ◈ | Research and investigation |

### Agent Integration

When using AI agents or automation tools, mark entries with the `--source agent` flag:

```bash
# Log from an AI agent
zdrive pane log my-feature "Completed refactoring task" --source agent

# Agent milestone with artifacts
zdrive pane log my-feature "Implemented new API endpoint" \
    --type milestone --source agent \
    --artifacts src/api/endpoint.rs tests/api_test.rs
```

This appears as `[🤖 AGENT]` in history output, making it easy to distinguish between human and agent work.

## Automated Snapshots with LLM

The `snapshot` command uses an LLM to automatically generate summaries from your work context:

```bash
# Generate a snapshot (requires consent and LLM configuration)
zdrive pane snapshot my-feature
```

### LLM Setup

1. **Grant consent** for sending context to LLM providers:

```bash
zdrive config consent --grant
```

2. **Configure a provider**:

```bash
# Option 1: Anthropic Claude
zdrive config set llm.provider anthropic
export ANTHROPIC_API_KEY=your-key

# Option 2: OpenAI
zdrive config set llm.provider openai
export OPENAI_API_KEY=your-key

# Option 3: Local Ollama (no API key needed)
zdrive config set llm.provider ollama
# Default endpoint: http://localhost:11434
```

3. **Optional: Set custom model**:

```bash
zdrive config set llm.model claude-sonnet-4-20250514
# Or for Ollama:
zdrive config set llm.model llama3.2
```

### Privacy & Security

- **Consent required**: Snapshot won't send data without explicit `consent --grant`
- **Secret filtering**: API keys, passwords, and tokens are automatically redacted
- **Local option**: Use Ollama for fully local, private operation
- **Revoke anytime**: `zdrive config consent --revoke`

## Shell Hooks for Automated Logging

Integrate `zdrive` with your shell to automatically log context at key moments.

### Zsh Integration

Add to your `~/.zshrc`:

```zsh
# Auto-snapshot on long-running command completion
# Uses the current directory name as pane name
zdrive_snapshot_on_complete() {
    local last_status=$?
    local elapsed=$SECONDS

    # Only snapshot after commands running >30 seconds
    if [[ $elapsed -gt 30 ]]; then
        local pane_name=$(basename "$PWD")
        zdrive pane snapshot "$pane_name" 2>/dev/null &!
    fi

    return $last_status
}

# Hook into command execution
preexec() { SECONDS=0 }
precmd() { zdrive_snapshot_on_complete }
```

### Bash Integration

Add to your `~/.bashrc`:

```bash
# Track command start time
zdrive_cmd_start() {
    ZDRIVE_CMD_START=${ZDRIVE_CMD_START:-$SECONDS}
}

# Snapshot after long commands
zdrive_cmd_complete() {
    local elapsed=$((SECONDS - ${ZDRIVE_CMD_START:-$SECONDS}))
    ZDRIVE_CMD_START=$SECONDS

    # Only snapshot after commands running >30 seconds
    if [[ $elapsed -gt 30 ]]; then
        local pane_name=$(basename "$PWD")
        zdrive pane snapshot "$pane_name" 2>/dev/null &
    fi
}

trap 'zdrive_cmd_start' DEBUG
PROMPT_COMMAND="${PROMPT_COMMAND:+$PROMPT_COMMAND;}zdrive_cmd_complete"
```

### Fish Integration

Add to your `~/.config/fish/config.fish`:

```fish
# Auto-snapshot on long command completion
function __zdrive_postexec --on-event fish_postexec
    set -l elapsed (math $CMD_DURATION / 1000)

    # Only snapshot after commands running >30 seconds
    if test $elapsed -gt 30
        set -l pane_name (basename $PWD)
        zdrive pane snapshot $pane_name 2>/dev/null &
    end
end
```

### Git Hook Integration

Create `.git/hooks/post-commit`:

```bash
#!/bin/bash
# Auto-log git commits as milestones

PANE_NAME=$(basename "$PWD")
COMMIT_MSG=$(git log -1 --format=%s)
FILES_CHANGED=$(git diff-tree --no-commit-id --name-only -r HEAD | head -5)

zdrive pane log "$PANE_NAME" "Committed: $COMMIT_MSG" \
    --type milestone \
    --source automated \
    --artifacts $FILES_CHANGED
```

Make it executable: `chmod +x .git/hooks/post-commit`

### CI/CD Integration

In your CI pipeline (e.g., GitHub Actions):

```yaml
- name: Log deployment milestone
  run: |
    zdrive pane log production "Deployed ${{ github.sha }}" \
      --type milestone \
      --source automated \
      --artifacts CHANGELOG.md
```

## Context Format for Agents

The `--format context` flag outputs LLM-optimized history for agent prompt injection:

```bash
# Get context for an AI agent (~1000 tokens)
zdrive pane history my-feature --format context
```

This produces a structured narrative including:
- Session overview with stats
- Recent activity (last 5 entries)
- Current state
- Key milestones
- Suggested next steps

## Pane Navigation

### Basic Commands

```bash
# Create or focus a pane
zdrive pane build

# Create pane in specific tab
zdrive pane logs --tab monitoring

# Attach metadata
zdrive pane api-server --tab backend --meta project=myapp

# Get pane info
zdrive pane info build
```

### Tab Management

```bash
# Create or switch to a tab
zdrive tab backend

# List all tracked panes
zdrive list

# Sync state with Zellij
zdrive reconcile
```

## Configuration

### Available Settings

| Key | Description | Default |
|-----|-------------|---------|
| `redis_url` | Redis connection URL | `redis://127.0.0.1:6379/` |

### Config Commands

```bash
# View all settings
zdrive config show

# Set a value
zdrive config set redis_url redis://localhost:6379/0
```

## Migration from v1.0

If upgrading from v1.0 (znav keyspace), migrate your data:

```bash
# Preview migration
zdrive migrate --dry-run

# Execute migration
zdrive migrate
```

## Architecture

- **CLI** (`src/cli.rs`) - Command parsing with clap
- **ZellijDriver** (`src/zellij.rs`) - Zellij action interface
- **StateManager** (`src/state.rs`) - Redis operations and intent history
- **Orchestrator** (`src/orchestrator.rs`) - Business logic coordination
- **OutputFormatter** (`src/output.rs`) - Human-readable formatting

### Redis Data Model

**Pane Hash**: `perth:pane:<name>`
- `session`, `tab`, `position`, timestamps, metadata

**Intent History**: `perth:pane:<name>:history`
- List of JSON-encoded IntentEntry objects (newest first)

## Development

```bash
# Run tests
cargo test

# Format code
cargo fmt

# Lint
cargo clippy
```

## License

MIT License - see LICENSE file for details.

## Acknowledgments

Built for developers who need contextual awareness across terminal sessions.
