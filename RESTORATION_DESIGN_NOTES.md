# Restoration Component Design Notes

**Created:** 2026-01-07
**Context:** Handoff from architecture session (started in wrong project directory)
**Status:** Architecture complete, ready for implementation

---

## Session Summary

This document captures context from a design session that added the Session Restoration Component (v2.1) to the Perth architecture.

### Origin Story

The session started debugging Zellij log spam:
```
ERROR |zellij_server::background| Failed to read created stamp of resurrection file:
Error { kind: Unsupported, message: "creation time is not available on this platform currently" }
```

**Root cause:** Zellij 0.42.2 was built with Rust <1.87, which didn't use `statx()` for birth time on Linux.

**Fix applied:** Built Zellij 0.44.0 from source with Rust 1.92.0, replaced binary at `/home/delorenj/.local/bin/zellij`.

This led to a discussion: since Perth already has Redis persistence and Zellij control, why not build a more robust restoration system than Zellij's built-in resurrection?

---

## What Was Added

### Architecture Document Update

**File:** `docs/architecture-perth-2026-01-05.md`
**Version:** 1.0 -> 1.1

Added new section: **Session Restoration Component (v2.1)** (lines 2068-2561)

Includes:
- 6 NFRs (NFR-REST-001 through NFR-REST-006)
- `RestorationManager` component design
- Data models: `SessionSnapshot`, `TabSnapshot`, `PaneSnapshot`, `RestoreReport`
- Redis schema: `perth:snapshots:{session}:{name}`
- CLI commands: `znav snapshot create|list|restore|delete|diff|daemon`
- 5 new FRs (FR-026 through FR-030)
- Trade-off decisions (7, 8, 9)
- Risk assessment

---

## Key Architectural Decisions

### Decision 7: Restoration in Core (not separate plugin)
- Restoration is a module within Perth, not a standalone Zellij plugin
- Shares StateManager, ZellijDriver, EventPublisher
- Module path: `src/restoration/`

### Decision 8: Incremental Snapshots
- Support both full and incremental (delta) snapshots
- Target <500 bytes for typical incremental
- Parent chaining for restoration

### Decision 9: Daemon for Auto-Snapshot
- `znav snapshot daemon --interval 10m` instead of shell hooks
- Avoids shell latency constraints (NFR-010: <10ms)
- Can be managed via systemd user unit

---

## Module Structure

```
src/
  restoration/           # NEW MODULE
    mod.rs              # RestorationManager
    capture.rs          # State capture logic
    restore.rs          # Restoration logic
    diff.rs             # Incremental diffing
    daemon.rs           # Auto-snapshot daemon
```

---

## NFRs for Restoration

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-REST-001 | Snapshot latency | <500ms |
| NFR-REST-002 | Structure fidelity | 100% tab/pane topology |
| NFR-REST-003 | Storage efficiency | <10KB delta |
| NFR-REST-004 | Graceful degradation | Continue on partial failure |
| NFR-REST-005 | Event integration | Bloodbank publish |
| NFR-REST-006 | Secret safety | Filter command history |

---

## CLI Commands

```bash
znav snapshot create <name> [--incremental]
znav snapshot list [--session <session>] [--format text|json]
znav snapshot restore <name> [--dry-run]
znav snapshot delete <name>
znav snapshot diff <snapshot-a> <snapshot-b>
znav snapshot daemon --interval 10m
```

---

## Redis Schema

```
perth:snapshots:{session}:{name}           # Hash: metadata
perth:snapshots:{session}:{name}:data      # String: JSON snapshot (gzip if >1KB)
perth:snapshots:{session}:index            # Sorted Set: by creation time
perth:snapshots:{session}:latest           # String: most recent snapshot name
```

TTL: 30 days (configurable via `restoration.snapshot_ttl_days`)

---

## Data Model Highlights

**PaneSnapshot captures:**
- `cwd: PathBuf` - working directory
- `running_command: Option<String>` - filtered for secrets
- `scroll_offset: u32`
- `git_branch: Option<String>`
- `imi_worktree: Option<String>` - iMi integration hook

**RestoreReport provides:**
- Success counts (tabs/panes restored)
- Warnings (CWD missing, command unavailable)
- Errors (fatal failures)
- Duration metrics

---

## Implementation Priority

Suggested order:
1. **capture.rs** - Get state capture working first
2. **mod.rs** - RestorationManager with basic snapshot/list
3. **restore.rs** - Basic restoration without incremental
4. **diff.rs** - Add incremental support
5. **daemon.rs** - Background auto-snapshot last

---

## Zellij Build Note

The Zellij binary was rebuilt from source to fix the birth time bug:

```bash
# Built with Rust 1.92.0
cd /tmp/zellij-build
cargo build --release
cp target/release/zellij ~/.local/bin/zellij
```

Upgraded from 0.42.2 to 0.44.0. Old binary backed up at `~/.local/bin/zellij.0.42.2.bak`.

Log location: `/tmp/zellij-1000/zellij-log/zellij.log`

---

## Next Steps

1. Run `/bmad:sprint-planning` to break FR-026 through FR-030 into stories
2. Start with `capture.rs` - use `zellij action dump-layout` for structure
3. Parse layout KDL for pane positions/geometry
4. Integrate with existing StateManager for Redis operations

---

## Questions Resolved

**Q: Plugin or module?**
A: Module within Perth. Shares existing drivers, single codebase.

**Q: Full or incremental snapshots?**
A: Both. Incremental by default with parent chaining.

**Q: Shell hook or daemon for auto-snapshot?**
A: Daemon. Avoids shell latency constraints.

---

*Generated from session in `/home/delorenj/code/utils/claude-notifications` - work saved to correct location via absolute paths.*
