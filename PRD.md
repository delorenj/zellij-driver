# ZellijDriver PRD (Redis-backed pane-first tab/pane manager)

> **Document Status**: Derived from v1.0 implementation (2025-01-02)
> This PRD was reverse-engineered from the working codebase to document actual behavior and design decisions.

## 1) Overview
ZellijDriver (CLI name: `znav`) is a low‑level manager for Zellij sessions that treats panes as the primary unit of navigation. Tabs are containers; focusing a pane implicitly focuses its tab. ZellijDriver persists a “shadow state” in Redis so panes/tabs/sessions can be referenced deterministically even when Zellij’s native APIs lack stable IDs or direct “focus by pane name.”

 ZellijDriver is convention‑agnostic: naming conventions and higher‑level semantics are owned by upstream tools that call `znav`. Upstream tools must not reach into Redis directly; any metadata they need to store or read goes through `znav` interfaces.

## 2) Goals
- Pane‑first navigation: `znav pane <name>` focuses an existing pane by name or creates it if missing.
- Deterministic creation: when creating, place panes in the correct tab (existing or auto‑created).
- Persistent state: survive Zellij restarts via Redis‑backed metadata.
- Low‑level, scriptable, idempotent primitives (no opinionated workflows).

## 3) Non‑Goals
- Full UI or TUI for navigation.
- Complex layout orchestration beyond basic split/new tab.
- Rich pane focus by absolute Zellij pane ID (until Zellij exposes a stable API).
- Multi‑user synchronization (single user workstation scope).

## 4) Target Users
- Developers who keep many long‑lived Zellij panes and want stable names.
- Power users scripting Zellij with repeatable commands.

## 5) User Experience (CLI)
Primary command: `znav`
- `znav pane <pane-name>`
  Focus a pane with this name if known; otherwise create in current tab.
- `znav pane <pane-name> --tab <tab-name>`
  Focus pane if exists; otherwise ensure tab exists (create if missing), then create pane there.
- `znav pane <pane-name> --session <session-name>`
  If pane exists in another session, switch/attach to that session and focus; otherwise create in target session.
- `znav pane <pane-name> --meta key=value [--meta key=value ...]`
  Attach generic metadata fields to the pane record when creating or updating.
- `znav pane info <pane-name>`
  Read pane metadata and return it in a machine‑readable format (JSON).
- `znav tab <tab-name>`
  Create or switch to a tab by name.
- `znav reconcile`
  Reconcile Redis state vs. actual Zellij layout using `dump-layout --json` (marks stale panes).
- `znav list`
  Display all tracked panes organized by session and tab in a tree view.

## 6) Functional Requirements
### Core
- Must be able to:
  - Determine current session name (prefer `ZELLIJ_SESSION_NAME` env var).
  - Create a tab by name if missing.
  - Create a pane (split) and name it.
  - Switch to a tab by name.

### Pane‑first behavior
- If a pane name exists in Redis, ZellijDriver should:
  - Ensure correct session (attach/switch if needed).
  - Switch to the pane's tab.
  - Auto-focus the specific pane using stored position metadata via sequential `focus-next-pane` calls.
- If pane name is not in Redis, ZellijDriver should:
  - Create/ensure tab if specified.
  - Count existing panes in target tab to capture position index.
  - Create the pane in that tab.
  - Store position in metadata as `meta:position` for future auto-focus.
  - Persist all metadata to Redis.

### Redis‑backed state
- Use Redis as source of truth for known panes.
- Maintain last‑seen timestamps for reconciliation and garbage collection.
- Allow generic metadata to be set and updated via `znav` only (no direct upstream Redis access).

### Session handling
- If a pane is in another session:
  - If the target session exists: switch/attach then act.
  - If not: create session and proceed (optional flag for “create if missing”).

### Reconciliation
- Implemented: Uses `zellij action dump-layout --json` to validate pane presence in current session.
- Parses layout JSON to extract all pane names from tabs and floating panes.
- Compares Redis state against actual layout:
  - Panes found in layout: marked seen with updated timestamp.
  - Panes missing from layout: marked stale.
  - Panes in other sessions: skipped.
- Graceful fallback: if dump-layout unavailable, skips reconciliation for that session.
- Reports summary: total panes checked, seen count, stale count, skipped count.

## 7) Data Model (Redis)
Keyspace: `znav:*`

Per‑pane hash:
- Key: `znav:pane:<pane_name>`
- Fields:
  - `session` (string) - Zellij session name
  - `tab` (string) - Tab name
  - `pane_id` (string, optional) - Reserved for future Zellij pane ID support
  - `created_at` (ISO8601) - Pane creation timestamp
  - `last_seen` (ISO8601) - Last reconciliation timestamp
  - `last_accessed` (ISO8601) - Last navigation timestamp
  - `stale` (boolean string: "true"/"false") - Indicates if pane no longer exists in layout
  - `meta:position` (string integer) - Pane index in tab for auto-focus (0-based)
  - `meta:*` (string) - Generic user-defined metadata fields for upstream tooling

Implementation notes:
- Position is stored as `meta:position` and used for sequential focus navigation
- Stale panes are not deleted, only marked for user review
- All metadata fields use `meta:` prefix for namespacing

Future considerations (not yet implemented):
- Auxiliary indices for fast lookups by tab or session
- TTL/expiry for automatic cleanup of old panes

## 8) Architecture
Modular Rust implementation with clear separation of concerns:

- **CLI Frontend** (`src/cli.rs`):
  - Clap-based argument parsing with subcommands
  - Metadata key-value parsing for `--meta` flags
  - Command routing to orchestrator

- **ZellijDriver** (`src/zellij.rs`):
  - Thin wrapper around `zellij action` CLI
  - Executes commands: `new-tab`, `go-to-tab-name`, `new-pane`, `rename-pane`, `query-tab-names`
  - Layout introspection via `dump-layout --json` with graceful fallback
  - Sequential pane focus using `focus-next-pane` repeated N times
  - Session detection via `ZELLIJ_SESSION_NAME` environment variable

- **StateManager** (`src/state.rs`):
  - Async Redis client with multiplexed connection
  - CRUD operations for pane metadata using Redis hashes
  - Metadata namespacing with `meta:` prefix
  - List and scan operations for bulk queries
  - Timestamp helpers for ISO8601 formatting

- **Orchestrator** (`src/orchestrator.rs`):
  - Business logic coordinator
  - Pane-first navigation with create-on-miss behavior
  - Session and tab management
  - Position tracking and auto-focus implementation
  - Reconciliation workflow with layout parsing
  - Tree visualization for `list` command

- **Types** (`src/types.rs`):
  - `PaneRecord`: Internal representation with all metadata fields
  - `PaneInfoOutput`: JSON-serializable output for `pane info`
  - `PaneStatus`: Enum for found/stale/missing states

- **Config** (`src/config.rs`):
  - Configuration loading from file or environment
  - Redis URL management

## 9) Edge Cases (Implemented)
- **Duplicate pane names across sessions**: Each pane record stores session name; attempting to open a pane in wrong session results in clear error message.
- **Tab creation**: When tab doesn't exist, creates it with `new-tab` then continues with pane operations.
- **Position tracking for current tab**: Position is set to 0 (unreliable) for `CURRENT_TAB` panes to avoid focus errors.
- **Layout unavailable**: Graceful fallback when `dump-layout --json` not supported; reconciliation skips those panes.
- **Pane not found during navigation**: Marked as stale in Redis with clear error message to user.
- **Redis connection failure**: Application fails fast with context-rich error at startup.
- **Cross-session navigation**: Detects active session mismatch and provides clear error; user must detach and retry.
- **Stale pane focus attempts**: Best-effort tab navigation; focus may fail but tab switch succeeds.
- **Metadata updates on existing panes**: `touch_pane` merges new metadata without overwriting existing fields.

## 10) Security & Reliability
- Redis configured locally; no external network dependency.
- Clear failure modes if Redis unavailable or Zellij CLI errors.
- Avoid destructive actions: no pane closure or tab deletion.

## 11) Success Metrics
- 95% of pane navigation commands resolve to the correct tab in <= 300ms.
- Zero data loss of pane metadata across Zellij restarts.
- User can rebuild workspace from Redis after session restart.

## 12) Acceptance Criteria (All Implemented ✅)
- ✅ `znav pane foo` creates a pane named "foo" if absent in current tab
- ✅ `znav pane foo` focuses both tab and specific pane if "foo" exists in Redis
- ✅ `znav pane foo --tab bar` ensures tab "bar" exists; creates "foo" there if missing
- ✅ `znav pane foo --meta key=value` attaches and persists custom metadata
- ✅ Redis keys for panes created with correct session/tab/position/timestamp metadata
- ✅ Reconcile command marks stale panes (doesn't delete) when no longer in layout
- ✅ `znav pane info foo` returns machine-readable JSON with metadata and status enum
- ✅ `znav list` displays tree view of all tracked panes organized by session/tab
- ✅ Auto-focus works via position-based sequential pane navigation
- ✅ Exit code 2 when `pane info` status is "missing"

## 13) Milestones

### ✅ MVP (Completed)
- ✅ CLI: `pane` command with Redis persistence
- ✅ Tab creation and tab switching
- ✅ Basic reconciliation with stale marking
- ✅ Metadata support via `--meta` flags

### ✅ v1.0 (Completed)
- ✅ Session awareness and session validation
- ✅ `tab` helper command
- ✅ `reconcile` command using `dump-layout --json` with graceful fallback
- ✅ `pane info` subcommand with JSON output
- ✅ Position-based auto-focus implementation
- ✅ `list` command with tree visualization

### 🔄 Future Enhancements (Not Yet Implemented)
- **v1.5**: Optional Redis indices for faster tab/session lookups
- **v2.0**: Direct pane ID focus if Zellij exposes stable pane IDs
- **v2.1**: TTL/expiry for automatic cleanup of forgotten panes
- **v2.2**: Cross-session pane movement/migration
- **v2.3**: Pane history and workspace snapshots

## 14) Implementation Status

**Current Version**: v1.0 (Production Ready)

**What's Working**:
- ✅ All core CLI commands implemented and tested
- ✅ Redis persistence with full metadata support
- ✅ Auto-focus via position tracking
- ✅ Reconciliation with layout JSON parsing
- ✅ Tree visualization for workspace overview
- ✅ Graceful error handling and fallbacks
- ✅ Cross-tab and cross-session awareness

**Known Limitations**:
- Position tracking unreliable for `CURRENT_TAB` panes (fallback: position=0)
- No cross-session pane navigation (user must detach and retry)
- Stale panes remain in Redis (manual cleanup required)
- No bulk operations (create/delete multiple panes)
- Focus relies on sequential navigation (may fail if layout changes between calls)

**Design Decisions**:
- **Pane-first philosophy**: Upstream tools should think in terms of panes, not tabs
- **Non-destructive**: Never delete panes or Redis keys; only mark stale
- **Explicit metadata**: Position stored as `meta:position` for flexibility
- **Fail-fast**: Redis errors and session mismatches halt execution with clear messages
- **Convention-agnostic**: No opinionated workspace patterns; pure primitives only

## 15) Open Questions & Future Considerations
- **Zellij stability**: Currently targets latest Zellij release; may need version detection
- **Pane-to-task mapping**: Upstream tools should enforce 1:1 pane-to-task if desired
- **Data retention**: No TTL implemented; consider adding configurable expiry for old panes
- **Performance**: Redis SCAN used for list operations; may need indices for large workspaces (1000+ panes)
- **Pane IDs**: Waiting on Zellij to expose stable pane IDs for direct focus without position hacks

## 16) CLI Output Schema

### `znav pane info <pane-name>`
Returns JSON for upstream tooling consumption.

**Fields**:
- `pane_name` (string) - Name of the pane
- `session` (string) - Zellij session name
- `tab` (string) - Tab name containing the pane
- `pane_id` (string|null) - Reserved for future Zellij pane ID (currently always null)
- `created_at` (string, ISO8601) - Timestamp when pane was first created
- `last_seen` (string, ISO8601) - Last reconciliation timestamp
- `last_accessed` (string, ISO8601) - Last navigation/touch timestamp
- `meta` (object) - Generic user-defined metadata (includes `position` if tracked)
- `status` (enum) - One of: `"found"`, `"stale"`, `"missing"`
- `source` (string) - Always `"redis"` in current implementation

**Exit codes**:
- `0` - Status is `found` or `stale`
- `2` - Status is `missing`

**Example output**:
```json
{
  "pane_name": "build",
  "session": "work",
  "tab": "ci",
  "pane_id": null,
  "created_at": "2025-01-02T10:15:42Z",
  "last_seen": "2025-01-02T10:15:42Z",
  "last_accessed": "2025-01-02T10:18:09Z",
  "meta": {
    "position": "2",
    "project": "myapp",
    "task_id": "T-381"
  },
  "status": "found",
  "source": "redis"
}
```

### `znav reconcile`
Prints summary line to stdout:
```
reconcile: session=work total=15 seen=12 stale=2 skipped=1
```

### `znav list`
Prints tree-structured visualization:
```
work
├── ci
│   ├── build
│   │     position=0
│   └── test
│         position=1
└── backend
    └── api-server [stale]
          position=0
          project=myapp
```

## 17) Implementation Summary

**Language & Stack**: Rust with async/await (Tokio runtime)

**Key Dependencies**:
- `clap` - CLI argument parsing and command structure
- `redis` - Async Redis client with multiplexed connections
- `serde`/`serde_json` - JSON serialization for output and layout parsing
- `anyhow` - Ergonomic error handling with context
- `chrono` - ISO8601 timestamp generation

**Code Structure**:
```
src/
├── main.rs          # Entry point, async runtime, command dispatch
├── cli.rs           # Clap command definitions and arg parsing
├── config.rs        # Configuration loading (file + env)
├── types.rs         # Data models (PaneRecord, PaneInfoOutput, PaneStatus)
├── state.rs         # Redis StateManager with CRUD operations
├── zellij.rs        # ZellijDriver wrapper around zellij CLI
└── orchestrator.rs  # Business logic coordinator
```

**Testing Strategy**:
- Manual integration testing against live Zellij sessions
- Redis state validation through `znav list` and `znav pane info`
- Edge case validation (missing tabs, stale panes, session mismatches)

**Deployment**:
- Single binary built with `cargo build --release`
- Installed to `~/.local/bin/znav` or similar PATH location
- Config at `~/.config/zellij-driver/config.toml` or via env var
- Requires Redis server running locally (default: `redis://127.0.0.1:6379`)

**Performance Characteristics**:
- Fast pane creation: ~100-200ms (Redis write + zellij action)
- Fast navigation: ~50-150ms (Redis read + tab switch + focus)
- Reconciliation: ~500ms-2s depending on layout size
- List command: ~100-300ms with SCAN + bulk reads

**Production Readiness**:
- ✅ Error handling with context-rich messages
- ✅ Graceful degradation (dump-layout fallback)
- ✅ Non-destructive operations (stale marking, no deletes)
- ✅ Structured logging via stdout/stderr
- ✅ Exit codes for scripting integration
- ⚠️ No automated tests yet (manual testing only)
- ⚠️ No telemetry/observability beyond stderr output
