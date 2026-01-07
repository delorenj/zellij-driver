# Sprint 5 Revised: Agentic Workspace Orchestration

**Date:** 2026-01-06
**Sprint Number:** 5
**Goal:** Enable Claude-driven parallel workflow orchestration with tab/pane management

---

## Sprint Goal

Complete the agentic workspace orchestration capabilities that enable Claude to programmatically create and manage Zellij tabs and panes for parallel development workflows. This sprint prioritizes the features needed for the target dev story: parallel PR fix implementation with correlation ID traceability.

---

## Story Revisions

### Kept from Original Sprint 5 (Aligned with Vision)

| ID | Title | Points | Priority | Rationale |
|----|-------|--------|----------|-----------|
| STORY-025 | Bloodbank Event Publisher | 5 | Should Have | Enables correlation ID from events |
| STORY-026 | Bloodbank Config | 2 | Should Have | Configuration for event integration |
| STORY-027 | History Type Filter | 2 | Should Have | Agent workflow filtering |

### New Stories (Vision-Driven)

| ID | Title | Points | Priority | Rationale |
|----|-------|--------|----------|-----------|
| STORY-036 | Tab Create with Correlation ID | 3 | Should Have | Traceability for agentic workflows |
| STORY-037 | Batch Pane Spawning | 5 | Should Have | Parallel fix implementation |
| STORY-038 | Claude Skill Documentation | 2 | Should Have | Skill discovery and usage |
| STORY-039 | Tab Naming Conventions | 2 | Could Have | Consistent naming patterns |

### Deferred to Sprint 6+ (Lower Priority)

| ID | Title | Points | Priority | Reason |
|----|-------|--------|----------|--------|
| STORY-033 | Goal Setting Command | 3 | Could Have | Not needed for target story |
| STORY-034 | Progress Estimation | 5 | Could Have | Not needed for target story |
| STORY-035 | Search Command | 8 | Could Have | Not needed for target story |

---

## Revised Sprint 5 Stories

### STORY-036: Tab Create with Correlation ID

**Epic:** EPIC-004 (Agent Integration)
**Priority:** Should Have
**Points:** 3

**User Story:**
As a Claude agent orchestrating parallel workflows,
I want to create tabs with correlation ID suffixes
So that I can trace work back to the triggering event.

**Acceptance Criteria:**
- [ ] `znav tab create <name> [--correlation-id <id>]` creates tab
- [ ] Tab name format: `{name}` or `{name}-{correlation_id}`
- [ ] Correlation ID stored in tab metadata
- [ ] Works with existing `znav tab` command
- [ ] Correlation ID queryable via `znav list --json`

**Technical Notes:**
- Add `Tab create` subcommand to CLI
- Store correlation ID in Redis hash for tab
- Keep existing tab navigation behavior

**Dependencies:** None

---

### STORY-037: Batch Pane Spawning

**Epic:** EPIC-004 (Agent Integration)
**Priority:** Should Have
**Points:** 5

**User Story:**
As a Claude agent,
I want to spawn multiple named panes in a single command
So that I can set up parallel workspaces efficiently.

**Acceptance Criteria:**
- [ ] `znav pane batch --tab <tab> --panes <p1,p2,p3>` spawns multiple panes
- [ ] Each pane is named according to the list
- [ ] Optional `--cwd <d1,d2,d3>` sets working directories
- [ ] Panes created in vertical split layout by default
- [ ] `--layout horizontal` option for horizontal splits
- [ ] All panes registered in Redis with position metadata

**Technical Notes:**
- Add `PaneBatch` command variant
- Loop over pane names calling existing `open_pane()`
- Position increments for each pane
- Use Zellij actions for layout control

**Dependencies:** None

---

### STORY-038: Claude Skill Documentation

**Epic:** EPIC-004 (Agent Integration)
**Priority:** Should Have
**Points:** 2

**User Story:**
As a Claude user,
I want a skill file documenting Perth capabilities
So that Claude can discover and use workspace management.

**Acceptance Criteria:**
- [x] SKILL.md in skill/ directory with frontmatter
- [x] Documents all CLI commands with examples
- [x] Includes agentic workflow patterns
- [x] References/ directory with CLI reference
- [ ] Installed to ~/.claude/skills/zellij-driver

**Technical Notes:**
- Already created: skill/SKILL.md
- Already created: skill/references/cli-reference.md
- Task: Add install script to mise tasks

**Dependencies:** None

---

### STORY-039: Tab Naming Conventions

**Epic:** EPIC-004 (Agent Integration)
**Priority:** Could Have
**Points:** 2

**User Story:**
As a developer using agentic workflows,
I want enforced tab naming conventions
So that workspaces are organized consistently.

**Acceptance Criteria:**
- [ ] `znav tab create` validates name format
- [ ] Suggested patterns: `{repo}(context)`, `{repo}(fixes)`
- [ ] Warning on non-conforming names (not blocking)
- [ ] `--strict` flag to enforce naming

**Technical Notes:**
- Regex validation for name patterns
- Configurable pattern in config file

**Dependencies:** STORY-036

---

### STORY-040: Restoration Data Model & Redis Schema

**Epic:** EPIC-006 (Session Restoration)
**Priority:** Should Have
**Points:** 3

**User Story:**
As a developer working on Perth,
I want well-defined restoration data types and Redis schema
So that session snapshots have consistent structure.

**Acceptance Criteria:**
- [ ] `SessionSnapshot`, `TabSnapshot`, `PaneSnapshot` structs in `types.rs`
- [ ] `RestoreReport`, `RestoreWarning` for restoration feedback
- [ ] Redis schema: `perth:snapshots:{session}:{name}` keyspace
- [ ] Implements `Serialize`/`Deserialize` for JSON and Redis
- [ ] Unit tests for serialization round-trip

**Technical Notes:**
- Foundation for restoration module (v2.1)
- Extends existing types.rs pattern
- See architecture doc lines 2134-2190

**Dependencies:** None (foundational)

---

## Sprint 5 Summary

| Category | Points |
|----------|--------|
| Kept Stories | 9 |
| New Stories | 12 |
| Restoration Foundation | 3 |
| **Total** | **24** |
| Capacity | 30 |
| Utilization | 80% |

**Buffer:** 6 points for testing, integration, and unforeseen complexity.

---

## Sprint 5 Story Order

1. **STORY-036**: Tab Create with Correlation ID (3 points)
   - Foundation for tab naming workflow

2. **STORY-037**: Batch Pane Spawning (5 points)
   - Core parallel workflow capability

3. **STORY-040**: Restoration Data Model & Redis Schema (3 points)
   - Foundation for v2.1 restoration feature

4. **STORY-025**: Bloodbank Event Publisher (5 points)
   - Event integration for correlation tracking

5. **STORY-026**: Bloodbank Config (2 points)
   - Configuration for events

6. **STORY-027**: History Type Filter (2 points)
   - Agent workflow filtering

7. **STORY-038**: Claude Skill Documentation (2 points)
   - Install script and verification

8. **STORY-039**: Tab Naming Conventions (2 points) - stretch goal
   - Polish for naming consistency

---

## Deliverables

By end of Sprint 5:

1. **Tab with Correlation ID:**
   ```bash
   znav tab create "myapp(fixes)" --correlation-id abc123
   # Creates tab named "myapp(fixes)-abc123"
   ```

2. **Batch Pane Creation:**
   ```bash
   znav pane batch --tab "myapp(fixes)" --panes fix-auth,fix-errors,fix-docs
   # Creates 3 panes in the tab, each named and tracked
   ```

3. **Bloodbank Integration:**
   ```bash
   # When milestone logged, event published to Bloodbank
   znav pane log fix-auth "Completed fix" --type milestone
   # → Event: perth.milestone.recorded
   ```

4. **Claude Skill:**
   ```bash
   # Skill installed and discoverable
   ls ~/.claude/skills/zellij-driver/
   # SKILL.md, references/
   ```

---

## Target Dev Story Validation

After Sprint 5, the following workflow should work:

```bash
# 1. Claude receives: "Fix the 3 issues from PR #42 in parallel"

# 2. Claude creates worktrees (via iMi skill)
imi add fix auth-refresh
imi add fix error-handling
imi add fix api-docs

# 3. Claude creates tab with correlation ID
znav tab create "myapp(fixes)" --correlation-id pr-42

# 4. Claude spawns panes for each fix
znav pane batch --tab "myapp(fixes)-pr-42" \
    --panes fix-auth,fix-errors,fix-docs \
    --cwd ../fix-auth-refresh,../fix-error-handling,../fix-api-docs

# 5. Each pane ready for Claude to work in
# Claude logs progress in each pane:
znav pane log fix-auth "Implementing token refresh" --source agent
```

---

---

## Sprint 6 Preview: Session Restoration (v2.1)

Based on the Session Restoration Component design (see `RESTORATION_DESIGN_NOTES.md`), Sprint 6 will complete the restoration feature.

### Sprint 6 Stories

| ID | Title | Points | Priority | Description |
|----|-------|--------|----------|-------------|
| STORY-041 | State Capture Module | 5 | Should Have | `capture.rs` - Parallel state capture from Zellij layout |
| STORY-042 | Snapshot Create & List | 5 | Should Have | CLI commands for creating and listing snapshots |
| STORY-043 | Restore Command | 5 | Should Have | CLI command for restoring from snapshot |
| STORY-044 | Incremental Snapshot Support | 3 | Should Have | Delta snapshots with parent chaining |
| STORY-045 | Snapshot Daemon | 3 | Could Have | Background auto-snapshot on interval |
| **Total** | | **21** | | |

### Sprint 6 Goal

Complete the Session Restoration feature (v2.1) enabling robust session persistence beyond Zellij's built-in resurrection:

```bash
# Create a snapshot before risky operation
znav snapshot create pre-refactor

# Later, restore if needed
znav snapshot restore pre-refactor --dry-run
znav snapshot restore pre-refactor

# Auto-snapshot daemon for continuous protection
znav snapshot daemon --interval 10m
```

### Implementation Priority (from design notes)

1. **capture.rs** - Get state capture working first
2. **mod.rs** - RestorationManager with basic snapshot/list
3. **restore.rs** - Basic restoration without incremental
4. **diff.rs** - Add incremental support
5. **daemon.rs** - Background auto-snapshot last

### NFRs for Restoration

| ID | Requirement | Target |
|----|-------------|--------|
| NFR-REST-001 | Snapshot latency | <500ms |
| NFR-REST-002 | Structure fidelity | 100% tab/pane topology |
| NFR-REST-003 | Storage efficiency | <10KB delta |
| NFR-REST-004 | Graceful degradation | Continue on partial failure |
| NFR-REST-005 | Event integration | Bloodbank publish |
| NFR-REST-006 | Secret safety | Filter command history |

---

## Migration from Original Sprint 5

**Stories Completed (Sprint 1-4):**
- All STORY-001 through STORY-024 implemented
- All STORY-028 through STORY-032 implemented
- All STORY-INF-* implemented

**New Sprint 5 Scope:**
- STORY-025, 026, 027 (from original)
- STORY-036, 037, 038, 039 (new)

**Moved to Sprint 6+:**
- STORY-033, 034, 035 (Could Have priority)

---

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Batch pane creation complexity | Start with simple sequential creation, optimize later |
| Correlation ID propagation | Store in Redis tab metadata, query via list |
| Bloodbank connectivity | Graceful degradation, optional dependency |

---

## Next Steps

1. Update `.bmad/sprint-status.yaml` with revised Sprint 5 stories
2. Update `docs/bmm-workflow-status.yaml` with new scope
3. Begin implementation with STORY-036

Run `/dev-story STORY-036` to start implementing tab correlation ID support.
