# Sprint 5 Target Story: Agentic Workspace Orchestration

## Epic
**EPIC-004: Agent Integration** (Should Have)

## User Story
> As a developer using Claude, I want Claude to programmatically manage my Zellij terminal workspace so that I can orchestrate parallel agentic workflows with full traceability.

## Acceptance Criteria

### AC-1: Claude Skill Integration
- [ ] A `zellij-driver` Claude skill is installed globally in `~/.claude/skills/`
- [ ] The skill provides clear instructions for tab/pane management
- [ ] Claude can successfully invoke `znav` commands via the skill

### AC-2: Tab Naming with Correlation ID
- [ ] Tabs can be named with format: `{repo}(fixes)` or `{repo}({context})`
- [ ] When triggered by Bloodbank event, tab name includes correlation ID suffix
- [ ] Format: `{repo}({context})-{correlation_id}` (e.g., `myapp(fixes)-abc123`)

### AC-3: Batch Pane Spawning
- [ ] A single command can spawn multiple panes in a tab
- [ ] Each pane can be named individually during batch creation
- [ ] Pane layout is organized (vertical splits by default)

### AC-4: Integration Workflow (Manual Test)
Given: A PR with 3 fixes needed
When: I describe the workflow to Claude
Then: Claude should:
1. Identify the 3 fixes from PR context
2. Create a tab named `zellij-driver(fixes)` (or with correlation ID if provided)
3. Spawn 3 panes in that tab, each named for a fix
4. Each pane is ready for a separate Claude session to work on

## Target Dev Story Flow

```
┌─────────────────────────────────────────────────────────────────────┐
│ User Prompt to Claude:                                              │
│ "Fix the 3 issues from PR #42 in parallel"                         │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│ Claude reads PR context (via gh CLI or MCP)                        │
│ Identifies: Fix-1, Fix-2, Fix-3                                    │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│ Claude uses iMi skill:                                              │
│ - git worktree add ../repo-fix-1 -b fix-1                          │
│ - git worktree add ../repo-fix-2 -b fix-2                          │
│ - git worktree add ../repo-fix-3 -b fix-3                          │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│ Claude uses zellij-driver skill:                                   │
│ - znav tab create "myapp(fixes)" --correlation-id abc123           │
│ - znav pane batch --tab "myapp(fixes)" \                           │
│     --panes fix-1,fix-2,fix-3 \                                    │
│     --cwd ../repo-fix-1,../repo-fix-2,../repo-fix-3               │
└─────────────────────────────────────────────────────────────────────┘
                                    │
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│ Claude spawns nested agents in each pane:                          │
│ Pane fix-1: "Implement the authentication fix..."                  │
│ Pane fix-2: "Refactor the error handling..."                       │
│ Pane fix-3: "Update the documentation..."                          │
└─────────────────────────────────────────────────────────────────────┘
```

## Implementation Notes

### Current State (v2.0)
- `znav pane <name>` - Creates a single pane
- `znav pane --tab <tab>` - Creates pane in specific tab
- Tab is auto-created if it doesn't exist
- No batch pane creation
- No correlation ID support

### Required Changes
1. **Tab command enhancement**: `znav tab create <name> [--correlation-id <id>]`
2. **Batch pane command**: `znav pane batch --tab <tab> --panes <p1,p2,p3> [--cwd <d1,d2,d3>]`
3. **Claude skill file**: Documents the workflow and command patterns

## Story Points
- Tab correlation ID: 3 points
- Batch pane spawning: 5 points
- Claude skill creation: 2 points
- Integration testing: 3 points
- **Total**: 13 points

## Dependencies
- STORY-025 (Bloodbank Event Publisher) provides correlation ID context
- iMi worktree skill (external) for worktree creation
- Claude Code skill system for skill discovery

## Future Integration Points
- **Yi**: Will orchestrate the high-level workflow
- **Flume**: Will manage agent spawning in panes
- **Jelmore**: Will handle correlation ID generation and traceability
- **Bloodbank**: Will publish events for workflow triggers
