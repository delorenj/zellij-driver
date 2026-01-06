---
created: 2026-01-03T06:00:00-05:00
category: Discussion
tags:
  - conversation-summary
  - intent-tracking
  - architecture
---
# Conversation Summary: Intent Tracking Revelation

## The Revelation

**User insight**: "Each tab/pane can already support metadata. It would be immensely valuable to have an account of what was being done last on the particular tab/pane... it's hard to think of a difference between what I want to see in this metadata and what I'd want to see in a git commit message. But where git is tied to the code state... this history is tied to my overall and short term goal and intention."

**Core realization**: Git tracks code state, but we need to track **work state** and **cognitive context**. The gap between:
- **Command history** (what the shell saw)
- **Intent history** (what you were trying to accomplish)

This is version control for human intent, not just code artifacts.

## The Problem Being Solved

**Current workflow** when resuming suspended work:
1. Read git log (code state only)
2. Grep shell history (commands without context)
3. Review modified files (what changed, not why)
4. Mental reconstruction of goals and progress
5. Resume work (with context-switching delay)

**Cognitive load**: High. **Time lost**: Significant.

## The Solution Architecture

### Transform znav from Navigation to Context Manager

**Layer 1** (v1.0 - Current): Navigation primitives
- Create/focus panes with metadata
- Persistent state across restarts

**Layer 2** (v2.0 - Proposed): Intent tracking
- Record cognitive context per session
- Query work narratives for recovery
- Linear history independent of git branches

**Layer 3** (v2.5 - Future): Agent integration
- LLM-powered auto-summarization
- Automatic checkpoint detection
- Context injection for session resumption

### Data Model

```
znav:pane:{name}:history → [
  {
    timestamp: "2026-01-02T15:45:32Z",
    summary: "Integrated OAuth library, added login endpoint",
    type: "milestone",
    artifacts: ["/src/api/auth.py", "/tests/test_auth.py"],
    source: "manual"
  },
  ...
]
```

**Key insight**: Store *summaries* (past-imperative, goal-oriented) not *raw data* (command logs).

## Git vs Pane History Comparison

| Aspect | Git Commits | Pane Intent History |
|--------|-------------|---------------------|
| Tracks | Code changes | Work context |
| History | Non-linear (branches) | Linear (per pane) |
| Scope | Versioned files | All activities (code, installs, config) |
| Use case | Code review, rollback | Context recovery, resumption |

**Example divergence**:
```
Git:
  abc123 - Add OAuth login
  def456 - Fix token refresh
  [branching, merging]

Pane:
  15:45 - Set up OAuth, researched libraries
  16:30 - Implemented login, debugging callbacks
  17:20 - Fixed refresh, installed redis-cli for debugging
  [Linear narrative]
```

## Implementation Phases

### Phase 1: Manual Logging
```bash
znav pane log backend-dev "Integrated OAuth library" --type milestone
znav pane history backend-dev
```

### Phase 2: Automated Snapshots
- LLM analyzes shell history + git diff
- Generates intent summary automatically
- Shell hook for periodic snapshots

### Phase 3: Jelmore Integration
- Agents record checkpoints
- Context recovery for resumption
- Bloodbank event publishing

### Phase 4: Advanced Features
- Semantic search across histories
- Goal state tracking
- Dashboard visualization

## Impact on 33GOD Ecosystem

### For Jelmore
- **Agent continuity**: Agents can resume with full context
- **Session recovery**: Narratives explain where work left off
- **Checkpoint tracking**: Milestones published to Bloodbank

### For Yi Agents
- **Context-aware task pickup**: Query history before starting work
- **Progress visibility**: See what's been done without reading code
- **Coordination**: Understand related work across team

### For Holocene
- **Timeline visualization**: Session narratives in dashboard
- **Progress tracking**: % complete estimates from milestones
- **Pattern recognition**: Common workflows and blockers

### For Developers
- **Instant resumption**: Read narrative, not git log
- **Reduced cognitive load**: Context preserved automatically
- **Team handoffs**: Share intent, not just code

## Why This Is Transformative

**User quote**: "This feels to me like one of potentially the most impactful pieces of the 33GOD ecosystem."

**Rationale**:
1. **Fills critical gap**: Between granular shell history and abstract git commits
2. **Reduces friction**: Context recovery from minutes to seconds
3. **Enables agents**: Narrative history is LLM-friendly resumption context
4. **Scales workflow**: From solo dev to multi-agent coordination
5. **Universal primitive**: Every component in 33GOD benefits

## Key Architectural Decisions

### 1. Linear History per Pane
- Not tied to git branches
- Survives pane recreation if same name
- Simple mental model: chronological narrative

### 2. Summary over Raw Data
- Store intent, not command logs
- LLM-generated or manually authored
- Human-readable and machine-queryable

### 3. Loose Coupling
- Jelmore queries via CLI, doesn't touch Redis
- Events published after znav operations
- Optional feature (disabled by default)

### 4. Privacy-First
- Filter secrets before LLM submission
- Local model fallback available
- Opt-in consent required

## Path Forward

1. ✅ Codify implementation plan
2. ✅ Compact conversation
3. ⬜ Rebrand to Bon Iver-ized name
4. ⬜ Update 33GOD docs to integrate new component
5. ⬜ BMAD methodology: PRD + architecture + sprint plan
6. ⬜ TDD development loop with specialized agents

## Philosophical Implications

This isn't just a feature addition. It's a fundamental rethinking of what "session management" means:

**Old paradigm**: Sessions are process containers
**New paradigm**: Sessions are **cognitive workspaces** with persistent memory

**Old question**: "Which pane is my code running in?"
**New question**: "What was I trying to accomplish, and how far did I get?"

This aligns with 33GOD's vision: AI-assisted workflows that reduce cognitive load and preserve intent across system boundaries.

## Next Steps

Proceed with rebranding and formal PRD creation using BMAD methodology.
