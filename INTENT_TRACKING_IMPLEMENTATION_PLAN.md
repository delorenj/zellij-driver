---
created: 2026-01-03T06:00:00-05:00
status: planning
category: Implementation
tags:
  - intent-tracking
  - context-persistence
  - cognitive-state
---
# Intent Tracking Implementation Plan

## Vision

Transform zellij-driver from navigation primitive to **cognitive context persistence layer** by tracking work intent alongside pane state. Enable resumable context across sessions through narrative history that captures *what was being worked on*, not just *what commands were run*.

## Problem Statement

**Current gap**: When resuming work after suspension, developers must reconstruct context by:
- Reading git commit history (code state only)
- Grepping shell history (commands without intent)
- Reviewing modified files (what changed, not why)
- Mental reconstruction of goals and progress

**Proposed solution**: Store intent history per pane as linear narrative, queryable for context recovery.

## Core Concept

**Git tracks code state. znav tracks work state.**

| Aspect | Git Commits | Pane Intent History |
|--------|-------------|---------------------|
| What it tracks | Code changes at specific points | Cognitive context across sessions |
| Granularity | File diffs with exact changes | Goals and progress summaries |
| History model | Non-linear (branches, merges) | Linear narrative per pane |
| Scope | Versioned artifacts only | All work: code, installs, config, debugging |
| Use case | Code review, rollback | Context recovery, session resumption |

## Data Model

### Redis Schema

```
# Existing pane metadata
znav:pane:{pane_name} → Hash {session, tab, position, meta:*, last_intent}

# New: Intent history (ordered list, newest first)
znav:pane:{pane_name}:history → List [
  {timestamp, summary, type, artifacts, commands_run, goal_delta, source},
  ...
]

# New: Artifact tracking (files touched)
znav:pane:{pane_name}:artifacts → Hash {
  "src/auth.py": "2026-01-02T15:45:32Z",
  ...
}
```

### Intent Entry Structure

```json
{
  "timestamp": "2026-01-02T15:45:32Z",
  "summary": "Integrated OAuth library, added login endpoint",
  "type": "milestone",  // milestone | checkpoint | exploration
  "artifacts": [
    "/src/api/auth.py",
    "/tests/test_auth.py",
    "/requirements.txt"
  ],
  "commands_run": 5,  // Count hint, not full history
  "goal_delta": "User authentication 60% complete",
  "source": "manual"  // manual | automated | agent
}
```

**Field rationale**:
- **summary**: Past-imperative like git commits, goal-oriented
- **type**: Distinguishes major milestones from exploratory work
- **artifacts**: Files touched (from git diff or file watches)
- **commands_run**: Context hint without full shell history
- **goal_delta**: Progress toward task completion (optional)
- **source**: Provenance (human, agent, auto-analysis)

## Implementation Phases

### Phase 1: Manual Logging (Effort: S, Risk: Low)

**Goal**: Basic intent recording via CLI

**Features**:
- `znav pane log <pane_name> <summary>` - Record intent manually
- `znav pane log <pane_name> <summary> --type milestone` - Classify type
- `znav pane log <pane_name> <summary> --artifacts file1 file2` - Attach files
- `znav pane history <pane_name>` - Query full history
- `znav pane history <pane_name> --last 5` - Recent entries only
- `znav pane history <pane_name> --format json` - Machine-readable output

**Redis operations**:
- LPUSH to `znav:pane:{name}:history` for ordered list
- HSET `znav:pane:{name}` field `last_intent` with latest summary
- TTL management (optional: 90-day expiry per entry)

**Rust implementation**:
```rust
// types.rs
pub struct IntentEntry {
    pub timestamp: String,
    pub summary: String,
    pub entry_type: IntentType,
    pub artifacts: Vec<String>,
    pub commands_run: Option<usize>,
    pub goal_delta: Option<String>,
    pub source: IntentSource,
}

pub enum IntentType {
    Milestone,
    Checkpoint,
    Exploration,
}

pub enum IntentSource {
    Manual,
    Automated,
    Agent,
}

// state.rs
impl StateManager {
    pub async fn log_intent(&mut self, pane_name: &str, entry: IntentEntry) -> Result<()>;
    pub async fn get_intent_history(&mut self, pane_name: &str, limit: Option<isize>) -> Result<Vec<IntentEntry>>;
}

// orchestrator.rs
impl Orchestrator {
    pub async fn log_intent(&mut self, pane_name: String, summary: String, opts: LogOptions) -> Result<()>;
    pub async fn get_history(&mut self, pane_name: String, opts: QueryOptions) -> Result<()>;
}
```

**CLI examples**:
```bash
# After completing OAuth integration
znav pane log backend-dev "Integrated OAuth library, added login endpoint" --type milestone

# Quick checkpoint during debugging
znav pane log backend-dev "Debugging token refresh flow, isolated issue to cache invalidation"

# Attach specific artifacts
znav pane log backend-dev "Refactored error handling" --artifacts src/errors.py src/middleware.py

# Query history
znav pane history backend-dev

# Last 3 entries as JSON
znav pane history backend-dev --last 3 --format json
```

**Success criteria**:
- Manual intent recording works reliably
- History persists across Zellij restarts
- JSON output enables scripting integration
- No breaking changes to existing `znav pane` functionality

### Phase 2: Automated Snapshots (Effort: M, Risk: Medium)

**Goal**: LLM-powered summarization of shell activity

**Features**:
- `znav pane snapshot <pane_name>` - Analyze recent activity and generate summary
- Shell history integration (last N commands)
- Git diff analysis (files changed, insertions/deletions)
- File modification timestamps
- LLM prompt engineering for intent extraction

**Architecture**:
```
znav pane snapshot
    ↓
1. Collect context
   - Shell history (last 20 commands via $HISTFILE)
   - Git diff --stat (if in git repo)
   - Recently modified files (last 30 min)
    ↓
2. Generate prompt
   "Analyze this session and summarize what was being worked on"
    ↓
3. Call LLM (Claude via API or local model)
    ↓
4. Parse response → IntentEntry
    ↓
5. Store in Redis with source=automated
```

**LLM integration**:
```rust
// llm.rs (new module)
pub async fn generate_intent_summary(context: SessionContext) -> Result<String> {
    let prompt = format!(
        "Analyze this terminal session and generate a concise summary (1-2 sentences, past-imperative):

        Shell commands:
        {}

        Files changed:
        {}

        Focus on the goal and outcome, not the specific commands.",
        context.shell_history.join("\n"),
        context.git_diff
    );

    // Call Claude API or local LLM
    let response = call_llm_api(prompt).await?;
    Ok(response.trim().to_string())
}
```

**Shell hook (optional)**:
```bash
# .zshrc integration
function znav_auto_snapshot() {
    local pane=$(znav pane current 2>/dev/null)
    if [[ -n "$pane" ]] && [[ $((RANDOM % 10)) -eq 0 ]]; then
        # 10% chance on each prompt = ~1 snapshot per 10 commands
        znav pane snapshot "$pane" &>/dev/null &
    fi
}

precmd_functions+=(znav_auto_snapshot)
```

**Configuration**:
```toml
# ~/.config/zellij-driver/config.toml
[intent_tracking]
enabled = true
llm_provider = "anthropic"  # anthropic | openai | local
llm_model = "claude-3-5-sonnet-20241022"
llm_api_key_env = "ANTHROPIC_API_KEY"
auto_snapshot_interval = 10  # commands between snapshots
```

**Success criteria**:
- LLM generates accurate summaries (>80% useful)
- API costs reasonable (<$0.01 per snapshot)
- No shell performance degradation
- Graceful fallback if LLM unavailable

### Phase 3: Jelmore Integration (Effort: M, Risk: Low)

**Goal**: Agent checkpoint publishing and context recovery

**Features**:
- Agents record milestones during work
- Session initialization creates first intent entry
- Context recovery feeds history to agent prompts
- Bloodbank event publishing on milestones

**Jelmore wrapper**:
```python
# jelmore/services/znav.py
async def log_milestone(pane_name: str, summary: str, metadata: dict):
    """Record agent milestone and publish event"""
    await run_command([
        "znav", "pane", "log", pane_name, summary,
        "--type", "milestone",
        "--source", "agent"
    ])

    # Publish to Bloodbank
    await bloodbank.publish("jelmore.events", {
        "event_type": "session.milestone",
        "pane_name": pane_name,
        "summary": summary,
        "metadata": metadata
    })

async def recover_context(task_id: str) -> str:
    """Get narrative history for agent resumption"""
    result = await run_command([
        "znav", "pane", "history", f"task-{task_id}",
        "--format", "json"
    ], capture_output=True)

    history = json.loads(result)

    # Format for agent prompt
    context = format_history_for_prompt(history)
    return context
```

**Agent prompt injection**:
```python
async def resume_coding_session(task_id: str):
    # Get pane info
    pane_info = await get_pane_info(f"task-{task_id}")

    # Get intent history
    context = await recover_context(task_id)

    # Feed to agent
    agent_prompt = f"""
    You are resuming work on task {task_id}.

    Previous session context:
    {context}

    Last checkpoint: {pane_info.get('last_intent', 'None')}

    Continue from where you left off.
    """

    await spawn_agent_with_context(agent_prompt)
```

**Success criteria**:
- Agents can record and query intent history
- Context recovery improves agent continuity
- Events published to Bloodbank correctly
- No Jelmore API changes required

### Phase 4: Advanced Features (Effort: L, Risk: Medium)

**Goal**: Semantic search, visualization, and intelligence

**Features**:
- **Semantic search**: Vector embeddings for intent similarity
- **Goal tracking**: Progress estimation toward task completion
- **Pattern recognition**: Identify recurring work types
- **Export**: Markdown/Obsidian format for journaling
- **Visualization**: Holocene dashboard integration

**Vector search**:
```rust
// Embed intent summaries for semantic search
pub async fn search_similar_intents(query: &str, limit: usize) -> Result<Vec<IntentEntry>> {
    let query_embedding = embed_text(query).await?;

    // Query Qdrant or similar
    let results = vector_db.search(query_embedding, limit).await?;

    Ok(results)
}
```

**Goal state tracking**:
```rust
pub struct GoalState {
    pub task_id: String,
    pub initial_goal: String,
    pub current_progress: f32,  // 0.0 to 1.0
    pub milestones: Vec<IntentEntry>,
    pub estimated_completion: Option<String>,
}
```

**Dashboard visualization**:
```typescript
// Holocene component
function SessionTimeline({ taskId }) {
    const history = useIntentHistory(taskId);

    return (
        <Timeline>
            {history.map(entry => (
                <TimelineItem
                    timestamp={entry.timestamp}
                    summary={entry.summary}
                    type={entry.type}
                    artifacts={entry.artifacts}
                />
            ))}
        </Timeline>
    );
}
```

**Success criteria**:
- Semantic search finds relevant context
- Progress tracking gives useful estimates
- Export integrates with existing workflows
- Dashboard provides clear visibility

## Technical Considerations

### LLM Provider Options

**Anthropic Claude** (Recommended):
- High-quality summarization
- Streaming API for responsiveness
- ~$0.003 per snapshot (3¢ per 1000)
- Requires API key

**OpenAI GPT**:
- Similar quality to Claude
- ~$0.002 per snapshot
- Requires API key

**Local models** (Ollama, llama.cpp):
- No API costs
- Privacy (no data leaves machine)
- Lower quality summaries
- Requires local GPU/resources

**Recommendation**: Start with Claude API for quality, add local fallback for privacy-sensitive users.

### Performance Targets

| Operation | Target Latency | Redis Ops |
|-----------|----------------|-----------|
| `znav pane log` | <50ms | 2 (LPUSH + HSET) |
| `znav pane history` | <100ms | 1 (LRANGE) |
| `znav pane snapshot` | <3s | 3-5 (with LLM call) |

### Storage Estimates

| Metric | Estimate |
|--------|----------|
| Bytes per intent entry | ~500 bytes (JSON) |
| Entries per pane | ~50 (90 days active) |
| Storage per pane | ~25KB |
| 100 active panes | ~2.5MB total |

**Conclusion**: Storage negligible, Redis easily handles scale.

### Privacy Considerations

**Shell history**:
- May contain secrets (API keys, passwords)
- Filter sensitive patterns before LLM submission
- Regex patterns: `(password|key|token|secret)=\S+`

**Git diffs**:
- May contain proprietary code
- Option to disable git analysis per-repo
- User consent required for LLM submission

**Configuration**:
```toml
[intent_tracking.privacy]
filter_secrets = true
secret_patterns = ["password=", "token=", "key="]
require_consent = true
disable_git_analysis = false
```

## Migration Path

### Backwards Compatibility

**Existing znav users**:
- No breaking changes to core commands
- Intent tracking disabled by default
- Opt-in via config or explicit `znav pane log`

**Redis schema**:
- New keys (`znav:pane:{name}:history`) don't conflict
- Existing `znav:pane:{name}` hashes gain one field: `last_intent`
- Field addition is backwards compatible

### Rollout Strategy

1. **Alpha** (Phase 1): Manual logging only, opt-in beta users
2. **Beta** (Phase 2): Automated snapshots, limited LLM usage
3. **GA** (Phase 3-4): Full integration, production-ready

## Success Metrics

### Adoption
- 50% of znav users enable intent tracking within 3 months
- Average 10+ intent entries per active pane

### Quality
- >80% of automated summaries rated "useful" by users
- <5% of summaries require manual correction

### Performance
- <100ms latency for history queries (p95)
- <$5/month LLM costs per power user (50 snapshots/day)

### Integration
- Jelmore adopts intent tracking for all agent sessions
- Holocene dashboard displays session timelines

## Risks and Mitigations

| Risk | Impact | Mitigation |
|------|--------|------------|
| LLM costs too high | Medium | Local model fallback, configurable limits |
| Poor summary quality | High | Prompt engineering, model selection, manual override |
| Privacy concerns | High | Secret filtering, opt-in consent, local-only mode |
| Performance degradation | Medium | Async processing, background snapshots, caching |
| Redis storage growth | Low | TTL on old entries, configurable history depth |

## Open Questions

1. **Snapshot trigger**: Time-based (every N minutes) vs command-based (every N commands)?
   - **Recommendation**: Command-based (every 10 commands) with debouncing

2. **History depth**: How many entries to retain per pane?
   - **Recommendation**: 90 days or 100 entries, whichever is less

3. **Cross-session narrative**: Should intent history span multiple pane lifecycles?
   - **Recommendation**: Yes, keyed by pane name regardless of session restarts

4. **Artifact tracking**: File watches vs git diff vs manual?
   - **Recommendation**: Git diff when available, file mtime fallback

5. **LLM streaming**: Real-time summary generation during work?
   - **Recommendation**: Defer to Phase 4, too complex for MVP

## Next Steps

1. ✅ Document this plan
2. ⬜ Update PRD with intent tracking features
3. ⬜ Prototype `znav pane log` in Rust
4. ⬜ Design LLM integration architecture
5. ⬜ Implement Phase 1 (manual logging)
6. ⬜ Alpha test with 33GOD team
7. ⬜ Iterate based on feedback
8. ⬜ Phase 2 implementation (automated snapshots)
9. ⬜ Jelmore integration (Phase 3)

## Appendix: Example Scenarios

### Scenario 1: Solo Developer

**Monday 14:00** - Start OAuth integration
```bash
znav pane backend-dev --tab auth
# Work for 2 hours
znav pane log backend-dev "Integrated OAuth library, added login endpoint" --type milestone
```

**Monday 16:00** - Debugging
```bash
# Work continues
znav pane log backend-dev "Debugging token refresh flow, isolated cache invalidation issue"
```

**Tuesday 09:00** - Resume
```bash
znav pane backend-dev
znav pane history backend-dev --last 5
# Output:
# 2026-01-01 16:00 - Debugging token refresh flow, isolated cache invalidation issue
# 2026-01-01 14:00 - Integrated OAuth library, added login endpoint
# ...
```

**Context recovered**: "I was debugging token refresh, found cache issue. Continue from there."

### Scenario 2: Agent-Driven Development

**Yi agent requests task**:
```python
# Jelmore spawns session
await create_coding_session(
    task_id="T-123",
    description="Add user authentication to API"
)

# Agent works, records checkpoints
await log_milestone("task-T-123", "Set up OAuth flow skeleton")
await log_milestone("task-T-123", "Implemented token generation and validation")
await log_milestone("task-T-123", "All tests passing, feature complete")
```

**Later: Different agent picks up related task**:
```python
# Query context from related pane
context = await recover_context("task-T-123")

# Agent prompt:
"""
Building on previous work:
- Set up OAuth flow skeleton
- Implemented token generation and validation
- All tests passing, feature complete

Now add refresh token endpoint...
"""
```

### Scenario 3: Team Handoff

**Developer A** works on feature, suspends:
```bash
znav pane feature-xyz "Refactored user model, added validation layer" --type milestone
znav pane feature-xyz "Updated tests to match new schema"
```

**Developer B** takes over:
```bash
znav pane feature-xyz
znav pane history feature-xyz
# Immediately understands: "Refactored model with validation, tests updated. Continue."
```

**No Slack messages, no git log archaeology, no context-switching delay.**

---

## Conclusion

Intent tracking transforms znav from **navigation tool** to **cognitive context manager**. By storing work narratives alongside pane state, we enable:

1. **Instant context recovery** after interruptions
2. **Agent continuity** across session boundaries
3. **Team coordination** through shared narratives
4. **Productivity insights** from work pattern analysis

This is the missing layer between shell history (too granular) and git commits (too focused on code). It's version control for **human intent**.

**Impact**: Reduced cognitive load, faster resumption, better agent coordination. Core infrastructure for 33GOD agentic workflows.
