# ZellijDriver PRD (Redis‑backed pane‑first tab/pane manager)

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
  Read pane metadata and return it in a machine‑readable format (e.g., JSON).
- `znav tab <tab-name>`
  Optional helper for creating or switching tabs; primarily a container API (pane‑first remains default).
- `znav reconcile`
  Reconcile Redis state vs. actual Zellij layout (GC of stale keys).

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
  - Switch to the pane’s tab.
  - Attempt focus (best‑effort; if no absolute focus by name, land in correct tab).
- If pane name is not in Redis, ZellijDriver should:
  - Create/ensure tab if specified.
  - Create the pane in that tab.
  - Persist metadata to Redis.

### Redis‑backed state
- Use Redis as source of truth for known panes.
- Maintain last‑seen timestamps for reconciliation and garbage collection.
- Allow generic metadata to be set and updated via `znav` only (no direct upstream Redis access).

### Session handling
- If a pane is in another session:
  - If the target session exists: switch/attach then act.
  - If not: create session and proceed (optional flag for “create if missing”).

### Reconciliation (MVP + v2)
- MVP: best‑effort; if pane in Redis cannot be found or action fails, mark it stale.
- v2: parse `zellij action dump-layout --json` to validate pane presence and tab membership.

## 7) Data Model (Redis)
Keyspace: `znav:*`

Per‑pane hash:
- Key: `znav:pane:<pane_name>`
- Fields:
  - `session` (string)
  - `tab` (string)
  - `pane_id` (string/int, optional future)
  - `created_at` (ISO8601)
  - `last_seen` (ISO8601)
  - `last_accessed` (ISO8601)
  - `meta:*` (string, optional; generic, user‑defined metadata fields for upstream tooling)

Auxiliary indices (optional v2):
- `znav:tab:<session>:<tab_name>` → set of pane names
- `znav:session:<session>` → set of pane names

Expiry:
- Optional TTL (e.g., 30 days) to auto‑clean forgotten panes.

## 8) Architecture
- **CLI Frontend**: argument parsing, command routing.
- **Zellij Driver**:
  - Executes `zellij action` commands:
    - `new-tab --name`
    - `go-to-tab-name`
    - `new-pane`
    - `rename-pane`
    - `dump-layout --json` (if available)
    - `query-tab-names`
- **State Manager (Redis)**:
  - CRUD for pane metadata.
  - Reconciliation helpers.
- **Orchestrator**:
  - Business logic: pane‑first navigation, create‑on‑miss, session switching.

## 9) Edge Cases
- Duplicate pane names across sessions: resolved by session namespace (pane name + session).
- Tab names missing: create with default pane, then rename or split.
- Zellij versions without JSON layout: fallback to reduced capabilities (tab‑level navigation and name persistence).
- Redis down: fail with clear error; optional “no‑redis” mode for transient fallback.

## 10) Security & Reliability
- Redis configured locally; no external network dependency.
- Clear failure modes if Redis unavailable or Zellij CLI errors.
- Avoid destructive actions: no pane closure or tab deletion.

## 11) Success Metrics
- 95% of pane navigation commands resolve to the correct tab in <= 300ms.
- Zero data loss of pane metadata across Zellij restarts.
- User can rebuild workspace from Redis after session restart.

## 12) Acceptance Criteria
- `znav pane foo` creates a pane named “foo” if absent.
- `znav pane foo` focuses the tab containing foo if present in Redis.
- `znav pane foo --tab bar` ensures tab “bar” exists; creates “foo” there if missing.
- Redis keys for panes created and updated with correct session/tab metadata.
- Reconcile command removes stale Redis entries when panes no longer exist.
- `znav pane info foo` returns machine‑readable JSON with pane metadata and status.

## 13) Milestones
### MVP
- CLI: `pane` command + Redis persistence.
- Tab creation and tab switching.
- Basic reconciliation (mark stale on failure).

### v1
- Session awareness and cross‑session navigation.
- `tab` helper command.
- `reconcile` command using `dump-layout --json` when available.

### v1.5
- Optional indices for tab/session lookups.
- Improved “focus pane” strategy (if Zellij adds pane ID focus).

## 14) Open Questions
- Zellij version: use latest Zellij release.
- Future rule: 1‑to‑1 mapping of pane to task, no duplicate tasks and therefore no duplicate panes.
- Caution on data loss: prefer preserving or warning over destructive actions when real pane state is unclear.

## 15) CLI Output Schema (pane info)
`znav pane info <pane-name>` returns JSON for upstream tooling. Fields:
- `pane_name` (string)
- `session` (string)
- `tab` (string)
- `pane_id` (string|null)
- `created_at` (string, ISO8601)
- `last_seen` (string, ISO8601)
- `last_accessed` (string, ISO8601)
- `meta` (object: string → string)
- `status` (string: `found` | `stale` | `missing`)
- `source` (string: `redis`)

Exit codes:
- `0` when status is `found` or `stale`
- `2` when status is `missing`

Example:
```json
{
  "pane_name": "build",
  "session": "work",
  "tab": "ci",
  "pane_id": null,
  "created_at": "2025-12-29T10:15:42-05:00",
  "last_seen": "2025-12-29T10:15:42-05:00",
  "last_accessed": "2025-12-29T10:18:09-05:00",
  "meta": {
    "project": "jelmore",
    "task_id": "T-381"
  },
  "status": "found",
  "source": "redis"
}
```
