# System Architecture: Perth

**Date:** 2026-01-05
**Architect:** delorenj
**Version:** 1.0
**Project Type:** Cognitive Context Management Tool
**Project Level:** Level 3
**Status:** Draft

---

## Document Overview

This document defines the system architecture for Perth, a cognitive context manager for Zellij terminal sessions. It provides the technical blueprint for implementation, addressing all 25 functional and 22 non-functional requirements from the PRD.

**Related Documents:**
- Product Requirements Document: `docs/prd-perth-2026-01-04.md`
- Implementation Plan: `INTENT_TRACKING_IMPLEMENTATION_PLAN.md`
- v1.0 Baseline: `PRD.md`

---

## Executive Summary

Perth extends the existing v1.0 navigation primitives with a cognitive context layer that captures work intent alongside pane state. The architecture follows a **Modular CLI Architecture** pattern, organizing functionality into cohesive modules with clear boundaries, unified by an orchestrator that coordinates state management, external integrations (Zellij, LLM providers), and event publishing.

**Key Architectural Decisions:**
1. **Rust CLI** - Continue with existing performant, type-safe foundation
2. **Redis as State Authority** - Shadow state pattern where Redis owns truth, Zellij renders
3. **Provider Abstraction for LLM** - Plugin architecture enabling Claude, GPT, local models
4. **Event-Driven Integration** - Bloodbank publishing for ecosystem coordination
5. **Layered Module Design** - Clear separation: CLI → Orchestrator → Services → Adapters

**Architecture Goals:**
- Maintain <100ms latency for core operations
- Enable phased feature rollout (Phase 1-4)
- Support offline/local-only mode without LLM
- Integrate seamlessly with 33GOD ecosystem (Jelmore, Bloodbank, Holocene)

---

## Architectural Drivers

These requirements heavily influence architectural decisions:

### Primary Drivers

| ID | Requirement | Architectural Impact |
|----|-------------|---------------------|
| NFR-001 | CLI commands <100ms (p95) | Async Redis ops, connection pooling, no blocking LLM in hot path |
| NFR-004 | 100% secret filtering before LLM | Dedicated filter pipeline, fail-closed design |
| NFR-008 | State persists across restarts | Redis AOF/RDB persistence, graceful reconnection |
| NFR-009 | Graceful LLM failure handling | Circuit breaker pattern, fallback to manual logging |
| NFR-014 | Modular Rust architecture | Clear module boundaries, trait-based abstractions |
| NFR-022 | Multiple LLM provider support | Provider trait with implementations per vendor |

### Secondary Drivers

| ID | Requirement | Architectural Impact |
|----|-------------|---------------------|
| NFR-002 | LLM snapshot <3s | Async execution, streaming responses, timeout handling |
| NFR-005 | User consent for LLM | Config-driven consent flag, first-run prompt |
| NFR-010 | No shell performance degradation | Async hooks, background processing, sub-10ms overhead |
| NFR-016 | Backwards compatibility with v1.0 | Non-breaking Redis schema additions, migration tooling |

---

## System Overview

### High-Level Architecture

Perth operates as a CLI tool that bridges user intent with Zellij terminal state through Redis persistence. The system has four primary concerns:

1. **Command Interface** - Clap-based CLI parsing and dispatch
2. **Business Logic** - Orchestration of navigation, intent logging, history queries
3. **State Management** - Redis-backed persistence with transaction support
4. **External Integrations** - Zellij actions, LLM providers, Bloodbank events

**Data Flow:**
```
User Command → CLI Parser → Orchestrator → [State Manager | Zellij Driver | LLM Provider]
                                ↓
                          Redis (State Authority)
                                ↓
                    Bloodbank Events (Optional)
```

### Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                              Perth CLI (znav)                               │
├─────────────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │    cli.rs   │  │  config.rs  │  │  types.rs   │  │      output.rs      │ │
│  │  (Clap CLI) │  │  (Settings) │  │ (Data Model)│  │  (Format/Display)   │ │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘ │
│         │                │                │                     │           │
│         └────────────────┴────────┬───────┴─────────────────────┘           │
│                                   ▼                                         │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │                        orchestrator.rs                                  ││
│  │  - open_pane()      - log_intent()      - get_history()                ││
│  │  - ensure_tab()     - snapshot()        - reconcile()                  ││
│  │  - visualize()      - search()          - migrate()                    ││
│  └───────────────────────────────┬─────────────────────────────────────────┘│
│                                  │                                          │
│         ┌────────────────────────┼────────────────────────┐                 │
│         ▼                        ▼                        ▼                 │
│  ┌─────────────┐         ┌─────────────┐         ┌─────────────────────┐   │
│  │  state.rs   │         │  zellij.rs  │         │       llm.rs        │   │
│  │  (Redis)    │         │ (Zellij CLI)│         │  (LLM Providers)    │   │
│  └──────┬──────┘         └──────┬──────┘         └──────────┬──────────┘   │
│         │                       │                           │               │
│  ┌──────┴──────┐         ┌──────┴──────┐         ┌──────────┴──────────┐   │
│  │    intent   │         │   actions   │         │     providers/      │   │
│  │   history   │         │   layout    │         │  ┌───────────────┐  │   │
│  │   metadata  │         │   focus     │         │  │   anthropic   │  │   │
│  └─────────────┘         └─────────────┘         │  │    openai     │  │   │
│                                                   │  │    local      │  │   │
│                                                   │  └───────────────┘  │   │
│                                                   └─────────────────────┘   │
│                                                                             │
│  ┌─────────────────────────────────────────────────────────────────────────┐│
│  │                           events.rs                                     ││
│  │                    (Bloodbank Integration)                              ││
│  └─────────────────────────────────────────────────────────────────────────┘│
└─────────────────────────────────────────────────────────────────────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
             ┌───────────┐   ┌───────────┐   ┌───────────┐
             │   Redis   │   │  Zellij   │   │ LLM APIs  │
             │  Server   │   │  Process  │   │ (Claude,  │
             │           │   │           │   │  GPT, etc)│
             └───────────┘   └───────────┘   └───────────┘
```

### Architectural Pattern

**Pattern:** Modular CLI Architecture with Service Adapters

**Rationale:**
- **CLI Tool Nature**: Perth is a command-line tool, not a web service. Microservices overhead unjustified.
- **Existing v1.0 Foundation**: Current Rust codebase already follows this pattern successfully.
- **Performance Requirements**: Direct function calls faster than IPC for <100ms target.
- **Team Size**: Single developer, modular monolith simpler to maintain than distributed system.
- **Phased Rollout**: Module boundaries enable incremental feature delivery.

**Pattern Characteristics:**
1. Single binary deployment
2. Trait-based abstraction for external services
3. Dependency injection via constructor parameters
4. Async/await for I/O operations
5. Clear module boundaries with explicit interfaces

---

## Technology Stack

### Runtime & Language

**Choice:** Rust 1.75+ with Tokio async runtime

**Rationale:**
- Existing v1.0 implementation in Rust (no migration cost)
- Memory safety without garbage collection pauses (supports <100ms latency)
- Strong type system catches errors at compile time
- Excellent async ecosystem (tokio, redis-rs, reqwest)
- Single binary deployment simplifies installation

**Trade-offs:**
- ✓ Gain: Performance, safety, existing codebase
- ✗ Lose: Slower iteration than Python/TS, steeper learning curve for contributors

### CLI Framework

**Choice:** Clap v4 with derive macros

**Rationale:**
- Already used in v1.0
- Excellent ergonomics with derive macros
- Auto-generated help and completion
- Strong typing for arguments

**Trade-offs:**
- ✓ Gain: Type-safe argument parsing, auto-help, shell completions
- ✗ Lose: Compile time (derive macros), binary size

### Database

**Choice:** Redis 6.0+ (existing)

**Rationale:**
- Already deployed in v1.0
- Sub-millisecond latency for state operations
- Native support for required data structures (Hash, List, String)
- AOF/RDB persistence for durability
- Widely deployed in 33GOD ecosystem

**Trade-offs:**
- ✓ Gain: Speed, existing integration, simple data model
- ✗ Lose: No complex queries, memory-bound storage, single point of failure

**Mitigation:** Redis Sentinel for HA in production deployments (optional).

### LLM Integration

**Choice:** Multi-provider abstraction with Anthropic Claude as primary

**Providers:**
1. **Anthropic Claude** (claude-3-5-sonnet) - Primary, best summarization quality
2. **OpenAI GPT** (gpt-4o-mini) - Alternative, lower cost
3. **Local Ollama** (llama3, mistral) - Privacy mode, no external calls

**Rationale:**
- Provider abstraction enables user choice
- Claude quality validated for code context summarization
- Local fallback addresses privacy concerns (NFR-007)
- Cost management through provider selection

**Trade-offs:**
- ✓ Gain: Flexibility, privacy options, vendor independence
- ✗ Lose: Implementation complexity, testing burden per provider

### HTTP Client

**Choice:** reqwest with rustls

**Rationale:**
- De facto Rust HTTP client
- Native async support
- TLS without OpenSSL dependency (easier cross-platform builds)

### Serialization

**Choice:** serde + serde_json

**Rationale:**
- Industry standard for Rust serialization
- Derive macros for zero-boilerplate
- Excellent JSON support for CLI output and API payloads

### Error Handling

**Choice:** anyhow + thiserror

**Rationale:**
- anyhow for application errors (context chaining, backtraces)
- thiserror for library-style typed errors where needed
- Existing pattern in v1.0

### Event Publishing

**Choice:** RabbitMQ via lapin (for Bloodbank integration)

**Rationale:**
- Bloodbank already uses RabbitMQ as event backbone
- lapin is mature async RabbitMQ client for Rust
- Loose coupling through events, not direct API calls

**Trade-offs:**
- ✓ Gain: Ecosystem integration, async event delivery
- ✗ Lose: Optional dependency, complexity for standalone use

### Development & Deployment

| Category | Choice | Rationale |
|----------|--------|-----------|
| Version Control | Git + GitHub | Existing, standard |
| CI/CD | GitHub Actions | Existing, good Rust support |
| Testing | cargo test + mockall | Standard Rust testing with mocks |
| Linting | clippy + rustfmt | Enforced code quality |
| Release | cargo-release + GitHub Releases | Automated versioning and publishing |
| Installation | cargo install, brew, AUR | Multiple distribution channels |

---

## System Components

### Component 1: CLI Module (`cli.rs`)

**Purpose:** Parse command-line arguments and dispatch to orchestrator

**Responsibilities:**
- Define CLI structure with Clap
- Validate arguments and flags
- Transform CLI args into domain commands
- Handle output formatting selection

**Interfaces:**
```rust
pub enum Command {
    Pane(PaneArgs),
    Tab(TabArgs),
    Reconcile,
    List,
    // v2.0 additions:
    Log(LogArgs),
    History(HistoryArgs),
    Snapshot(SnapshotArgs),
    Search(SearchArgs),
    Migrate,
}
```

**Dependencies:** None (entry point)

**FRs Addressed:** All (entry point for all commands)

---

### Component 2: Orchestrator (`orchestrator.rs`)

**Purpose:** Coordinate business logic across services

**Responsibilities:**
- Implement command handlers
- Coordinate state, Zellij, and LLM operations
- Manage transaction boundaries
- Handle error recovery and fallbacks

**Interfaces:**
```rust
pub struct Orchestrator {
    state: StateManager,
    zellij: ZellijDriver,
    llm: Box<dyn LLMProvider>,
    events: Option<EventPublisher>,
}

impl Orchestrator {
    // v1.0 navigation
    pub async fn open_pane(&mut self, ...) -> Result<()>;
    pub async fn ensure_tab(&mut self, ...) -> Result<()>;
    pub async fn reconcile(&mut self) -> Result<()>;
    pub async fn visualize(&mut self) -> Result<()>;

    // v2.0 intent tracking
    pub async fn log_intent(&mut self, ...) -> Result<()>;
    pub async fn get_history(&mut self, ...) -> Result<Vec<IntentEntry>>;
    pub async fn snapshot(&mut self, ...) -> Result<IntentEntry>;
    pub async fn search(&mut self, ...) -> Result<Vec<SearchResult>>;

    // Migration
    pub async fn migrate(&mut self) -> Result<MigrationReport>;
}
```

**Dependencies:** StateManager, ZellijDriver, LLMProvider, EventPublisher

**FRs Addressed:** FR-001 through FR-025 (orchestrates all features)

---

### Component 3: State Manager (`state.rs`)

**Purpose:** Manage Redis state persistence

**Responsibilities:**
- CRUD operations for pane metadata
- Intent history storage and retrieval
- Artifact tracking
- Schema migration
- Connection pooling and error handling

**Interfaces:**
```rust
pub struct StateManager {
    pool: Pool<RedisConnectionManager>,
}

impl StateManager {
    // Pane metadata (v1.0)
    pub async fn get_pane(&mut self, name: &str) -> Result<Option<PaneRecord>>;
    pub async fn set_pane(&mut self, record: &PaneRecord) -> Result<()>;
    pub async fn list_panes(&mut self) -> Result<Vec<PaneRecord>>;

    // Intent history (v2.0)
    pub async fn log_intent(&mut self, pane: &str, entry: &IntentEntry) -> Result<()>;
    pub async fn get_history(&mut self, pane: &str, limit: Option<i64>) -> Result<Vec<IntentEntry>>;
    pub async fn get_last_intent(&mut self, pane: &str) -> Result<Option<String>>;

    // Artifacts (v2.0)
    pub async fn track_artifacts(&mut self, pane: &str, files: &[PathBuf]) -> Result<()>;
    pub async fn get_artifacts(&mut self, pane: &str) -> Result<HashMap<String, String>>;

    // Migration
    pub async fn migrate_keyspace(&mut self, from: &str, to: &str) -> Result<usize>;
}
```

**Dependencies:** Redis connection pool

**FRs Addressed:** FR-001, FR-002, FR-004, FR-020, FR-021, FR-023, FR-024

---

### Component 4: Zellij Driver (`zellij.rs`)

**Purpose:** Interface with Zellij terminal multiplexer

**Responsibilities:**
- Execute `zellij action` commands
- Parse layout dumps
- Navigate tabs and panes
- Focus management

**Interfaces:**
```rust
pub struct ZellijDriver;

impl ZellijDriver {
    pub fn new_tab(&self, name: &str) -> Result<()>;
    pub fn go_to_tab(&self, name: &str) -> Result<()>;
    pub fn new_pane(&self, name: &str, direction: Option<Direction>) -> Result<()>;
    pub fn rename_pane(&self, name: &str) -> Result<()>;
    pub fn focus_next_pane(&self) -> Result<()>;
    pub fn dump_layout(&self) -> Result<LayoutDump>;

    // v2.0: Context capture support
    pub fn get_pane_cwd(&self) -> Result<Option<PathBuf>>;
}
```

**Dependencies:** Zellij CLI (external process)

**FRs Addressed:** FR-020, FR-022, FR-024, FR-025

---

### Component 5: LLM Provider (`llm.rs`)

**Purpose:** Abstract LLM interactions for intent summarization

**Responsibilities:**
- Define provider trait
- Implement Anthropic, OpenAI, Local providers
- Handle secret filtering before API calls
- Manage rate limiting and retries
- Parse and validate LLM responses

**Interfaces:**
```rust
#[async_trait]
pub trait LLMProvider: Send + Sync {
    async fn summarize(&self, context: &SessionContext) -> Result<String>;
    fn name(&self) -> &str;
    fn is_available(&self) -> bool;
}

pub struct AnthropicProvider { api_key: String, model: String }
pub struct OpenAIProvider { api_key: String, model: String }
pub struct LocalProvider { endpoint: String, model: String }
pub struct NoOpProvider; // Fallback when LLM disabled

pub struct SessionContext {
    pub shell_history: Vec<String>,
    pub git_diff: Option<String>,
    pub modified_files: Vec<PathBuf>,
    pub pane_name: String,
    pub previous_intent: Option<String>,
}
```

**Dependencies:** reqwest (HTTP), Config (API keys)

**FRs Addressed:** FR-006, FR-009

---

### Component 6: Secret Filter (`filter.rs`)

**Purpose:** Sanitize sensitive data before LLM submission

**Responsibilities:**
- Apply configurable regex patterns
- Redact matching content
- Log filter activity for audit
- Fail closed on filter errors

**Interfaces:**
```rust
pub struct SecretFilter {
    patterns: Vec<Regex>,
    replacement: String,
}

impl SecretFilter {
    pub fn new(patterns: &[String]) -> Result<Self>;
    pub fn filter(&self, content: &str) -> FilterResult;
    pub fn filter_context(&self, context: &mut SessionContext);
}

pub struct FilterResult {
    pub filtered: String,
    pub redaction_count: usize,
    pub patterns_matched: Vec<String>,
}
```

**Dependencies:** regex crate

**FRs Addressed:** FR-008

---

### Component 7: Event Publisher (`events.rs`)

**Purpose:** Publish events to Bloodbank for ecosystem integration

**Responsibilities:**
- Connect to RabbitMQ
- Serialize and publish events
- Handle connection failures gracefully
- Support async fire-and-forget publishing

**Interfaces:**
```rust
pub struct EventPublisher {
    connection: Option<Connection>,
    channel: Option<Channel>,
    exchange: String,
}

impl EventPublisher {
    pub async fn connect(url: &str) -> Result<Self>;
    pub async fn publish(&self, event: PerthEvent) -> Result<()>;
    pub fn is_connected(&self) -> bool;
}

pub enum PerthEvent {
    MilestoneRecorded {
        pane_name: String,
        summary: String,
        timestamp: DateTime<Utc>,
        artifacts: Vec<String>,
    },
    SessionStarted { ... },
    SessionEnded { ... },
}
```

**Dependencies:** lapin (RabbitMQ client), Bloodbank

**FRs Addressed:** FR-012

---

### Component 8: Config Manager (`config.rs`)

**Purpose:** Load and manage configuration

**Responsibilities:**
- Load config from file and environment
- Provide defaults for all settings
- Validate configuration
- Support runtime config queries

**Interfaces:**
```rust
pub struct Config {
    // Redis
    pub redis_url: String,

    // Intent tracking
    pub intent_tracking: IntentConfig,

    // LLM
    pub llm: LLMConfig,

    // Privacy
    pub privacy: PrivacyConfig,

    // Events
    pub bloodbank_url: Option<String>,
}

pub struct IntentConfig {
    pub enabled: bool,
    pub auto_snapshot_interval: usize,
    pub history_depth: usize,
    pub history_ttl_days: u64,
}

pub struct LLMConfig {
    pub provider: LLMProviderType,
    pub anthropic_api_key: Option<String>,
    pub openai_api_key: Option<String>,
    pub local_endpoint: Option<String>,
    pub model: Option<String>,
    pub timeout_secs: u64,
}

pub struct PrivacyConfig {
    pub filter_secrets: bool,
    pub secret_patterns: Vec<String>,
    pub require_consent: bool,
    pub consent_given: bool,
}
```

**Dependencies:** serde, toml, directories crate

**FRs Addressed:** FR-007, FR-009, FR-010, NFR-005, NFR-006, NFR-007

---

### Component 9: Output Formatter (`output.rs`)

**Purpose:** Format command output for display

**Responsibilities:**
- Render text output (human-readable)
- Render JSON output (machine-readable)
- Color-code by entry type
- Handle terminal width

**Interfaces:**
```rust
pub enum OutputFormat {
    Text,
    Json,
    Context, // Optimized for agent prompt injection
}

pub struct OutputFormatter {
    format: OutputFormat,
    color_enabled: bool,
}

impl OutputFormatter {
    pub fn format_history(&self, entries: &[IntentEntry]) -> String;
    pub fn format_pane_info(&self, info: &PaneInfoOutput) -> String;
    pub fn format_tree(&self, tree: &SessionTree) -> String;
    pub fn format_search_results(&self, results: &[SearchResult]) -> String;
}
```

**Dependencies:** colored, serde_json

**FRs Addressed:** FR-005, FR-011, FR-014, NFR-017, NFR-018

---

## Data Architecture

### Data Model

**Core Entities:**

```
┌─────────────────────────────────────────────────────────────────┐
│                           Session                               │
│  (Implicit - derived from Zellij session)                       │
│  - Has many: Tabs                                               │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                             Tab                                 │
│  (Implicit - derived from Zellij tab)                          │
│  - Has many: Panes                                              │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                            Pane                                 │
│  id: String (pane name, unique per session)                     │
│  session: String                                                │
│  tab: String                                                    │
│  position: u32                                                  │
│  status: PaneStatus (active | stale | missing)                 │
│  last_intent: Option<String>                                   │
│  metadata: HashMap<String, String>                              │
│  created_at: DateTime                                           │
│  - Has many: IntentEntries                                     │
│  - Has many: Artifacts                                          │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                        IntentEntry                              │
│  id: Uuid                                                       │
│  pane_name: String                                              │
│  timestamp: DateTime<Utc>                                       │
│  summary: String (1-2 sentences, past imperative)              │
│  entry_type: IntentType (milestone | checkpoint | exploration) │
│  artifacts: Vec<String> (file paths)                           │
│  commands_run: Option<usize>                                   │
│  goal_delta: Option<String>                                    │
│  source: IntentSource (manual | automated | agent)             │
└─────────────────────────────────────────────────────────────────┘
```

### Database Design (Redis Schema)

**Keyspace: `perth:*`** (migrated from `znav:*`)

```
# Pane Metadata (Hash)
perth:pane:{pane_name}
  Fields:
    session     -> String (session name)
    tab         -> String (tab name)
    position    -> u32 (pane position for focus)
    status      -> String (active|stale|missing)
    last_intent -> String (latest intent summary, for quick display)
    created_at  -> String (ISO 8601 timestamp)
    meta:{key}  -> String (user-defined metadata)

# Intent History (List, newest first)
perth:pane:{pane_name}:history
  Elements: JSON-serialized IntentEntry objects
  Operations:
    LPUSH - Add new entry (newest first)
    LRANGE 0 N - Get last N entries
    LTRIM 0 99 - Maintain max 100 entries

# Artifact Tracking (Hash)
perth:pane:{pane_name}:artifacts
  Fields:
    {file_path} -> String (last modified timestamp)

# Global Indices (Sets)
perth:panes                    -> Set of all pane names
perth:sessions:{session}:panes -> Set of panes in session
perth:tabs:{session}:{tab}:panes -> Set of panes in tab

# Configuration (Hash)
perth:config
  Fields:
    consent_given -> bool
    keyspace_version -> u32
```

**Index Strategy:**
- Primary access pattern: by pane name (O(1) hash lookup)
- Secondary: by session/tab (set membership)
- History: ordered list with LRANGE for pagination

**TTL Strategy:**
- Intent entries: 90 days (configurable via `history_ttl_days`)
- Pane metadata: No TTL (persists until explicit delete or stale cleanup)
- Apply TTL to individual list elements via background job

### Data Flow

**Write Path (Intent Logging):**
```
User: znav pane log backend "Fixed auth bug"
  │
  ▼
CLI parses → Orchestrator.log_intent()
  │
  ├─► StateManager.log_intent()
  │     LPUSH perth:pane:backend:history {entry_json}
  │     HSET perth:pane:backend last_intent "Fixed auth bug"
  │
  └─► EventPublisher.publish() [if milestone]
        Publish to Bloodbank: perth.milestone.recorded
```

**Read Path (History Query):**
```
User: znav pane history backend --last 5
  │
  ▼
CLI parses → Orchestrator.get_history(limit=5)
  │
  ▼
StateManager.get_history()
  LRANGE perth:pane:backend:history 0 4
  │
  ▼
OutputFormatter.format_history(entries)
  │
  ▼
stdout (text or JSON)
```

**Snapshot Path (LLM Summarization):**
```
User: znav pane snapshot backend
  │
  ▼
CLI parses → Orchestrator.snapshot()
  │
  ├─► Collect context:
  │     - Shell history (last 20 commands)
  │     - Git diff --stat
  │     - Recently modified files
  │
  ├─► SecretFilter.filter_context()
  │     - Redact passwords, tokens, keys
  │
  ├─► LLMProvider.summarize(context)
  │     - Call Claude/GPT/Local
  │     - Parse response
  │
  ├─► StateManager.log_intent(automated)
  │     - Store generated summary
  │
  └─► EventPublisher.publish() [if milestone type]
```

---

## API Design

### API Architecture

Perth is a CLI tool, not a web service. The "API" is the command-line interface.

**CLI Command Structure:**
```
znav <subcommand> [arguments] [flags]
```

**Design Principles:**
- Consistent flag patterns across commands
- JSON output available for all commands (`--format json`)
- Exit codes: 0=success, 1=error, 2=not found
- Stderr for errors, stdout for output

### CLI Commands (Endpoints)

#### Navigation Commands (v1.0)

```bash
# Create or navigate to pane
znav pane <name> [--tab <tab>] [--session <session>] [--meta key=value...]

# Get pane info
znav pane info <name>

# Ensure tab exists
znav tab <name>

# Reconcile Redis with Zellij state
znav reconcile

# List all panes as tree
znav list
```

#### Intent Tracking Commands (v2.0)

```bash
# Manual intent logging
znav pane log <name> "<summary>" [--type milestone|checkpoint|exploration] [--artifacts file1 file2...] [--source manual|agent]

# Query intent history
znav pane history <name> [--last N] [--format text|json|context] [--type milestone]

# Auto-generate snapshot via LLM
znav pane snapshot <name> [--type milestone|checkpoint]

# Semantic search (Phase 4)
znav search "<query>" [--limit N] [--format text|json]

# Set pane goal (Phase 4)
znav pane goal <name> "<goal_description>"
znav pane progress <name>
```

#### Migration Commands

```bash
# Migrate keyspace from znav:* to perth:*
znav migrate [--dry-run]
```

#### Configuration Commands

```bash
# Show current config
znav config show

# Set config value
znav config set <key> <value>

# Grant LLM consent
znav config consent --grant
```

### Output Formats

**Text (default):**
```
$ znav pane history backend --last 3

Intent History: backend
───────────────────────────────────
[milestone] 2 hours ago
  Fixed cache invalidation in token refresh

[checkpoint] 4 hours ago
  Debugging token refresh flow, isolated issue

[milestone] yesterday
  Integrated OAuth library, added login endpoint
───────────────────────────────────
```

**JSON (`--format json`):**
```json
{
  "pane_name": "backend",
  "entries": [
    {
      "id": "abc123",
      "timestamp": "2026-01-05T14:30:00Z",
      "summary": "Fixed cache invalidation in token refresh",
      "entry_type": "milestone",
      "artifacts": ["src/auth/tokens.py"],
      "source": "manual"
    }
  ]
}
```

**Context (`--format context`):**
```
Previous work on backend:
- Fixed cache invalidation in token refresh (2 hours ago)
- Debugging token refresh flow, isolated issue (4 hours ago)
- Integrated OAuth library, added login endpoint (yesterday)

Last checkpoint: "Fixed cache invalidation in token refresh"
Artifacts: src/auth/tokens.py

Suggested next step: Continue with authentication feature completion
```

### Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Resource not found (pane, history) |
| 3 | Configuration error |
| 4 | Redis connection error |
| 5 | LLM provider error |

---

## Non-Functional Requirements Coverage

### NFR-001: Command Latency

**Requirement:** CLI commands complete in <100ms for p95 (excluding LLM operations)

**Architecture Solution:**
- **Redis connection pooling** via r2d2 or bb8 - eliminates connection overhead per command
- **Async I/O** with Tokio - non-blocking Redis operations
- **No LLM in hot path** - snapshot command is explicitly slow, other commands avoid LLM
- **Minimal dependencies** - lean startup time, no heavy frameworks

**Implementation Notes:**
- Connection pool size: 4-8 connections
- Redis timeout: 500ms (fail fast)
- Pre-compiled regex for secret filtering

**Validation:**
- Benchmark suite measuring p50, p95, p99 for all commands
- CI gate: fail if `znav pane history` p95 > 100ms

---

### NFR-002: LLM Snapshot Performance

**Requirement:** LLM-powered snapshot completes in <3s (p95)

**Architecture Solution:**
- **Streaming responses** from Claude/GPT where supported
- **Timeout handling** with 10s hard timeout, 3s soft timeout
- **Async execution** - non-blocking during LLM call
- **Context size limits** - cap shell history at 20 commands, git diff at 50 lines

**Implementation Notes:**
- Use streaming API for Anthropic (server-sent events)
- Implement circuit breaker after 3 consecutive failures
- Cache recent snapshots to avoid duplicate calls

**Validation:**
- Monitor snapshot latency in production
- Alert if p95 exceeds 5s

---

### NFR-003: History Query Performance

**Requirement:** History queries return in <100ms regardless of history size (up to 100 entries)

**Architecture Solution:**
- **Redis LRANGE** - O(S+N) where S=start, N=count; ~O(1) for recent entries
- **Fixed history depth** - max 100 entries per pane, LTRIM on insert
- **No pagination for typical queries** - 100 entries fits in single response

**Implementation Notes:**
- Default query: LRANGE 0 9 (last 10)
- Full history: LRANGE 0 99 (all 100)
- JSON serialization ~1ms for 100 entries

**Validation:**
- Benchmark with 100-entry histories
- CI gate: p95 < 100ms

---

### NFR-004: Secret Filtering Reliability

**Requirement:** 100% of configured patterns filtered before LLM submission

**Architecture Solution:**
- **Fail-closed design** - if filter fails, abort LLM call entirely
- **Pre-compiled regex** - validate patterns at config load, fail startup if invalid
- **Defense in depth** - filter at SessionContext construction, not just before API call
- **Audit logging** - log every redaction with pattern name (not content)

**Implementation Notes:**
```rust
impl SecretFilter {
    pub fn filter_context(&self, context: &mut SessionContext) -> Result<()> {
        // Fail-closed: any filter error aborts
        for cmd in &mut context.shell_history {
            *cmd = self.filter(cmd)?;
        }
        if let Some(diff) = &mut context.git_diff {
            *diff = self.filter(diff)?;
        }
        Ok(())
    }
}
```

**Default Patterns:**
```regex
(password|passwd|pwd)\s*[=:]\s*\S+
(api_?key|apikey)\s*[=:]\s*\S+
(secret|token)\s*[=:]\s*\S+
(auth|bearer)\s+\S+
AWS_[A-Z_]+=\S+
```

**Validation:**
- Unit tests for each default pattern
- Fuzz testing with known secret formats
- Integration test: verify no secrets in LLM request logs

---

### NFR-005: User Consent for LLM

**Requirement:** LLM features require explicit opt-in consent

**Architecture Solution:**
- **Consent flag in config** - `privacy.consent_given: bool`
- **First-run prompt** - interactive consent request on first `snapshot` command
- **Revocable** - `znav config consent --revoke`
- **Clear explanation** - show exactly what data is sent

**Implementation Notes:**
```rust
impl Orchestrator {
    pub async fn snapshot(&mut self, pane: &str) -> Result<IntentEntry> {
        if !self.config.privacy.consent_given {
            return Err(anyhow!(
                "LLM consent required. Run: znav config consent --grant\n\
                 This will send shell history and git diff to {}",
                self.llm.name()
            ));
        }
        // ... proceed with snapshot
    }
}
```

**Validation:**
- Test: snapshot fails without consent
- Test: consent flag persists across restarts

---

### NFR-006: Opt-In/Opt-Out Granularity

**Requirement:** Intent tracking features independently disable-able

**Architecture Solution:**
- **Hierarchical config:**
  ```toml
  [intent_tracking]
  enabled = true           # Master switch

  [llm]
  provider = "anthropic"   # Can be "none" to disable LLM

  [privacy]
  require_consent = true
  ```
- **Per-command opt-out** - `--no-tracking` flag (future)

**Validation:**
- Test: `enabled = false` disables all intent commands
- Test: `llm.provider = "none"` allows manual logging, blocks snapshot

---

### NFR-007: Local-Only Mode

**Requirement:** System fully functional without external API calls

**Architecture Solution:**
- **NoOpProvider** - LLM provider that always returns error
- **Offline Redis** - local Redis server for development
- **No network requirement** - manual logging works without any external calls

**Implementation Notes:**
```rust
pub struct NoOpProvider;

impl LLMProvider for NoOpProvider {
    async fn summarize(&self, _: &SessionContext) -> Result<String> {
        Err(anyhow!("LLM disabled. Use manual logging: znav pane log <name> <summary>"))
    }
    fn is_available(&self) -> bool { false }
}
```

**Validation:**
- Integration test: full workflow with `llm.provider = "none"`
- Network isolation test: works with network disabled

---

### NFR-008: State Persistence Across Restarts

**Requirement:** All pane metadata and intent history survive restarts

**Architecture Solution:**
- **Redis persistence** - AOF (append-only file) for durability
- **Graceful reconnection** - retry logic on Redis connection loss
- **State reconciliation** - `znav reconcile` syncs with Zellij after restart

**Implementation Notes:**
- Recommended Redis config: `appendonly yes`, `appendfsync everysec`
- Connection retry: exponential backoff, max 5 retries

**Validation:**
- Test: kill Redis, restart, verify data intact
- Test: kill Zellij, restart, verify pane history preserved

---

### NFR-009: Graceful LLM Failure Handling

**Requirement:** LLM failures handled gracefully, system falls back to manual logging

**Architecture Solution:**
- **Circuit breaker** - after 3 failures, disable auto-snapshot for 5 minutes
- **Timeout handling** - 10s timeout with clear error message
- **Fallback messaging** - suggest manual logging on failure
- **No cascading failures** - LLM errors isolated to snapshot command

**Implementation Notes:**
```rust
pub struct CircuitBreaker {
    failures: AtomicU32,
    last_failure: AtomicU64,
    threshold: u32,
    cooldown_secs: u64,
}

impl CircuitBreaker {
    pub fn is_open(&self) -> bool {
        let failures = self.failures.load(Ordering::Relaxed);
        if failures < self.threshold {
            return false;
        }
        // Check if cooldown elapsed
        let now = unix_timestamp();
        let last = self.last_failure.load(Ordering::Relaxed);
        now - last < self.cooldown_secs
    }
}
```

**Validation:**
- Test: simulate LLM timeout, verify graceful error
- Test: verify circuit breaker opens after threshold

---

### NFR-010: No Shell Performance Degradation

**Requirement:** Shell hooks add <10ms overhead to prompt rendering

**Architecture Solution:**
- **Async background execution** - hook spawns detached process
- **Fire-and-forget** - don't wait for response
- **Rate limiting** - max 1 snapshot per 10 commands
- **Fast path check** - skip hook if tracking disabled

**Implementation Notes:**
```bash
# .zshrc hook
function _perth_hook() {
    # Early exit if disabled
    [[ -z "$PERTH_ENABLED" ]] && return

    # Increment counter, snapshot every 10 commands
    (( _PERTH_CMD_COUNT++ ))
    if (( _PERTH_CMD_COUNT % 10 == 0 )); then
        # Background, detached, no output
        ( znav pane snapshot "$PERTH_CURRENT_PANE" &>/dev/null & ) &>/dev/null
    fi
}
```

**Validation:**
- Benchmark: measure prompt latency with/without hook
- Gate: hook overhead < 10ms

---

### NFR-014: Modular Rust Architecture

**Requirement:** Codebase organized into modules with clear separation

**Architecture Solution:**
- **Module per concern:**
  ```
  src/
    main.rs          # Entry point
    cli.rs           # Command parsing
    config.rs        # Configuration
    orchestrator.rs  # Business logic coordination
    state.rs         # Redis state management
    zellij.rs        # Zellij driver
    llm.rs           # LLM provider abstraction
    llm/
      anthropic.rs
      openai.rs
      local.rs
    filter.rs        # Secret filtering
    events.rs        # Bloodbank integration
    output.rs        # Output formatting
    types.rs         # Data types
    error.rs         # Error types
  ```
- **Trait-based abstractions** - `LLMProvider` trait enables provider swapping
- **Dependency injection** - Orchestrator receives services via constructor

**Validation:**
- Each module has dedicated unit tests
- No circular dependencies (enforced by compiler)
- Module interfaces documented

---

### NFR-016: Backwards Compatibility with v1.0

**Requirement:** v2.0 adds new Redis keys without breaking existing schema

**Architecture Solution:**
- **Additive schema changes only:**
  - New keys: `perth:pane:{name}:history`, `perth:pane:{name}:artifacts`
  - New field on existing hash: `last_intent`
- **Keyspace migration tool** - `znav migrate` handles `znav:*` → `perth:*`
- **Deprecation period** - support both keyspaces for 6 months

**Implementation Notes:**
```rust
impl StateManager {
    pub async fn migrate_keyspace(&mut self) -> Result<MigrationReport> {
        // 1. Scan znav:* keys
        // 2. Copy to perth:* with transformation
        // 3. Verify integrity
        // 4. Report statistics
    }
}
```

**Validation:**
- Test: v1.0 data accessible after upgrade
- Test: migration is idempotent

---

### NFR-020: Zellij Version Compatibility

**Requirement:** Support Zellij v0.39.0 and later

**Architecture Solution:**
- **Version check on startup** - parse `zellij --version`
- **Feature detection** - verify required actions available
- **Clear error messaging** - if version too old, show upgrade instructions

**Implementation Notes:**
```rust
impl ZellijDriver {
    pub fn check_version() -> Result<Version> {
        let output = Command::new("zellij").arg("--version").output()?;
        let version = parse_version(&output.stdout)?;
        if version < Version::new(0, 39, 0) {
            return Err(anyhow!(
                "Zellij v0.39.0+ required (found {}). Please upgrade.",
                version
            ));
        }
        Ok(version)
    }
}
```

**Validation:**
- CI tests against Zellij 0.39, 0.40, latest
- Version check fails gracefully on missing Zellij

---

### NFR-022: Multiple LLM Provider Support

**Requirement:** Support Claude, GPT, and local models via configurable provider

**Architecture Solution:**
- **Provider trait** with common interface
- **Factory function** creates provider from config
- **Provider-specific config** nested under `[llm]`

**Implementation Notes:**
```rust
pub fn create_provider(config: &LLMConfig) -> Box<dyn LLMProvider> {
    match config.provider {
        LLMProviderType::Anthropic => {
            Box::new(AnthropicProvider::new(
                config.anthropic_api_key.clone().expect("API key required"),
                config.model.clone().unwrap_or("claude-3-5-sonnet-20241022".into()),
            ))
        }
        LLMProviderType::OpenAI => {
            Box::new(OpenAIProvider::new(
                config.openai_api_key.clone().expect("API key required"),
                config.model.clone().unwrap_or("gpt-4o-mini".into()),
            ))
        }
        LLMProviderType::Local => {
            Box::new(LocalProvider::new(
                config.local_endpoint.clone().unwrap_or("http://localhost:11434".into()),
                config.model.clone().unwrap_or("llama3".into()),
            ))
        }
        LLMProviderType::None => Box::new(NoOpProvider),
    }
}
```

**Validation:**
- Unit tests for each provider
- Integration test with mock LLM server

---

## Security Architecture

### Authentication

Perth is a local CLI tool - no user authentication required. Security boundaries are:

1. **File system permissions** - config file protected by OS
2. **Redis authentication** - optional `requirepass` for Redis server
3. **LLM API keys** - stored in config or environment variables

**API Key Storage:**
- Prefer environment variables: `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`
- Config file fallback with 600 permissions
- Never log or display API keys

### Authorization

No authorization model needed - single-user local tool.

For multi-user scenarios (future):
- Redis ACLs for namespace isolation
- Per-user keyspace prefix

### Data Encryption

**At Rest:**
- Redis: Use TLS and authentication if exposed to network
- Config file: OS file permissions (600)
- Shell history: Relies on existing shell security

**In Transit:**
- LLM API calls: HTTPS (TLS 1.2+) via reqwest/rustls
- Redis: TLS optional, recommended for remote Redis
- Bloodbank: TLS for RabbitMQ connections

### Security Best Practices

**Secret Filtering (Critical):**
- Filter shell history before LLM submission
- Fail-closed on filter errors
- Configurable patterns for custom secrets

**Input Validation:**
- Pane names: alphanumeric, dash, underscore only
- Summary text: max 500 characters
- File paths: validated, no command injection

**Dependency Security:**
- Minimal dependencies
- cargo-audit in CI
- Dependabot for vulnerability alerts

**No Elevated Privileges:**
- Runs as user, no sudo required
- No setuid/setgid
- Respects $HOME, $XDG_CONFIG_HOME

---

## Scalability & Performance

### Scaling Strategy

Perth is a local CLI tool - traditional scaling not applicable. Performance focus areas:

**Single-Machine Scaling:**
- Support 100+ panes per session
- Support 100 intent entries per pane
- Total data: ~5MB for typical usage

**Data Growth Management:**
- LTRIM history to 100 entries on insert
- TTL expiry for old entries (90 days default)
- Reconcile command cleans stale panes

### Performance Optimization

**Redis Operations:**
- Connection pooling (4-8 connections)
- Pipelining for multi-key operations
- Minimal round trips per command

**Startup Time:**
- Lazy initialization of LLM provider
- Config caching
- Pre-compiled regex patterns

**Memory Usage:**
- Stream large outputs instead of buffering
- Bounded history queries
- No global state accumulation

### Caching Strategy

**No Application-Level Caching:**
- Redis is already in-memory cache
- Commands are short-lived
- Caching adds complexity for marginal gain

**Redis-Level Caching:**
- Connection pool reuses connections
- Redis caches hot keys in memory

### Load Balancing

Not applicable for CLI tool. Each command is independent process.

---

## Reliability & Availability

### High Availability Design

**Single-User Mode (Default):**
- No HA required
- Redis runs locally
- Failure = local machine failure

**Shared Redis Mode (Optional):**
- Redis Sentinel for automatic failover
- Connection retry on failover
- Graceful degradation if Redis unavailable

### Disaster Recovery

**RPO (Recovery Point Objective):** 1 second
- Redis AOF with `appendfsync everysec`
- Intent data persisted immediately

**RTO (Recovery Time Objective):** 30 seconds
- Restart Redis, reconnect
- No complex recovery procedures

### Backup Strategy

**Recommended:**
- Redis RDB snapshots: hourly
- AOF persistence: every second
- External backup: daily Redis dump to S3/local

**Recovery:**
- Restore RDB file to Redis data directory
- Start Redis, verify data

### Monitoring & Alerting

**Metrics to Track:**
- Command latency (p50, p95, p99)
- Redis connection health
- LLM API error rate
- Snapshot success rate

**Logging Strategy:**
```rust
// Structured logging with tracing
#[instrument(skip(self), fields(pane = %pane_name))]
pub async fn log_intent(&mut self, pane_name: &str, entry: IntentEntry) -> Result<()> {
    tracing::info!(entry_type = ?entry.entry_type, "logging intent");
    // ...
}
```

**Log Levels:**
- ERROR: Failures requiring attention
- WARN: Degraded operation (LLM timeout, Redis retry)
- INFO: Normal operations (command executed)
- DEBUG: Detailed execution (Redis commands, API calls)

**Alerting (for shared deployments):**
- Redis unavailable > 30 seconds
- LLM error rate > 10%
- Command latency p95 > 500ms

---

## Integration Architecture

### External Integrations

**Zellij Terminal Multiplexer:**
- Integration: Shell out to `zellij action` commands
- Stability: Stable CLI interface
- Failure mode: Commands fail if Zellij not running
- Version: v0.39.0+

**Anthropic Claude API:**
- Integration: REST API via reqwest
- Auth: API key in header
- Failure mode: Timeout, rate limit, server error
- Fallback: Local model or manual logging

**OpenAI API:**
- Integration: REST API via reqwest
- Auth: API key in header
- Failure mode: Same as Claude
- Fallback: Claude or local model

**Ollama (Local LLM):**
- Integration: REST API to localhost
- Auth: None (local)
- Failure mode: Connection refused if not running
- Fallback: Manual logging only

### Internal Integrations (33GOD Ecosystem)

**Bloodbank (Event Bus):**
- Integration: RabbitMQ via lapin
- Exchange: `perth.events`
- Events: `perth.milestone.recorded`, `perth.session.started`, `perth.session.ended`
- Failure mode: Events dropped if unavailable (non-blocking)

**Jelmore (Session Orchestrator):**
- Integration: Jelmore calls `znav` CLI commands
- Data: Jelmore reads intent history via `--format json`
- Coupling: Loose (CLI interface), no direct code dependency

**Holocene (Dashboard):**
- Integration: Bloodbank events → Holocene timeline widget
- Data: Event payloads contain pane, summary, timestamp
- Coupling: None (event-driven)

### Message/Event Architecture

**Event Schema:**
```json
{
  "event_type": "perth.milestone.recorded",
  "version": "1.0",
  "timestamp": "2026-01-05T10:30:00Z",
  "payload": {
    "pane_name": "backend-dev",
    "summary": "Implemented user authentication",
    "entry_type": "milestone",
    "artifacts": ["src/auth.py", "tests/test_auth.py"],
    "source": "manual"
  }
}
```

**Event Flow:**
```
Perth CLI → EventPublisher → RabbitMQ (Bloodbank) → Consumers
                                                      ├─► Holocene (dashboard)
                                                      ├─► Yi (orchestrator)
                                                      └─► Flume (session manager)
```

---

## Development Architecture

### Code Organization

```
zellij-driver/
├── Cargo.toml
├── Cargo.lock
├── README.md
├── LICENSE
├── PRD.md                          # v1.0 PRD (baseline)
├── docs/
│   ├── prd-perth-2026-01-04.md     # v2.0 PRD
│   └── architecture-perth-2026-01-05.md  # This document
├── src/
│   ├── main.rs                     # Entry point, async runtime setup
│   ├── cli.rs                      # Clap CLI definition
│   ├── config.rs                   # Configuration loading
│   ├── orchestrator.rs             # Business logic coordination
│   ├── state.rs                    # Redis state manager
│   ├── zellij.rs                   # Zellij driver
│   ├── llm/
│   │   ├── mod.rs                  # LLMProvider trait
│   │   ├── anthropic.rs            # Anthropic implementation
│   │   ├── openai.rs               # OpenAI implementation
│   │   ├── local.rs                # Ollama/local implementation
│   │   └── noop.rs                 # NoOp fallback
│   ├── filter.rs                   # Secret filtering
│   ├── events.rs                   # Bloodbank integration
│   ├── output.rs                   # Output formatting
│   ├── types.rs                    # Data types, IntentEntry, etc.
│   └── error.rs                    # Error types
├── tests/
│   ├── integration/
│   │   ├── pane_test.rs
│   │   ├── intent_test.rs
│   │   └── snapshot_test.rs
│   └── fixtures/
│       └── test_config.toml
└── .github/
    └── workflows/
        ├── ci.yml
        └── release.yml
```

### Module Structure

**Dependency Graph:**
```
main.rs
  └─► cli.rs
        └─► orchestrator.rs
              ├─► state.rs
              ├─► zellij.rs
              ├─► llm/*.rs
              ├─► filter.rs
              ├─► events.rs
              └─► output.rs

config.rs ◄─── (all modules)
types.rs  ◄─── (all modules)
error.rs  ◄─── (all modules)
```

**Interface Boundaries:**
- cli → orchestrator: Command enums
- orchestrator → state: Async methods returning Result<T>
- orchestrator → llm: LLMProvider trait
- orchestrator → events: EventPublisher struct

### Testing Strategy

**Unit Testing (Target: 80% coverage):**
- Each module has `#[cfg(test)]` section
- Mock external dependencies (Redis, LLM, Zellij)
- Property-based tests for data transformations

**Integration Testing:**
- Real Redis (testcontainers or local)
- Mock LLM server (wiremock)
- Zellij not mocked (requires real terminal)

**E2E Testing:**
- Shell scripts exercising full CLI flows
- Run in CI with Zellij installed

**Performance Testing:**
- Benchmark suite with criterion
- CI gates for latency regressions

**Testing Infrastructure:**
```rust
// Mock LLM provider for tests
pub struct MockLLMProvider {
    response: String,
    should_fail: bool,
}

impl LLMProvider for MockLLMProvider {
    async fn summarize(&self, _: &SessionContext) -> Result<String> {
        if self.should_fail {
            Err(anyhow!("Mock failure"))
        } else {
            Ok(self.response.clone())
        }
    }
}
```

### CI/CD Pipeline

```yaml
# .github/workflows/ci.yml
name: CI

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    services:
      redis:
        image: redis:6
        ports:
          - 6379:6379
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run tests
        run: cargo test --all-features
      - name: Run clippy
        run: cargo clippy -- -D warnings
      - name: Check formatting
        run: cargo fmt -- --check

  benchmark:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run benchmarks
        run: cargo bench --no-run  # Just compile to verify

  release:
    needs: [test]
    if: startsWith(github.ref, 'refs/tags/')
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Build release
        run: cargo build --release
      - name: Create release
        uses: softprops/action-gh-release@v1
```

---

## Deployment Architecture

### Environments

**Development:**
- Local Redis (docker or native)
- `llm.provider = "none"` or local Ollama
- Debug logging enabled

**Testing:**
- CI Redis (testcontainers)
- Mock LLM server
- Full test suite

**Production (user machines):**
- User's Redis server
- User's LLM API keys
- Release binary

### Deployment Strategy

**Installation Methods:**
1. **cargo install** - `cargo install perth` (from crates.io)
2. **GitHub Releases** - pre-built binaries for Linux/macOS
3. **Homebrew** - `brew install 33god/tap/perth`
4. **AUR** - `yay -S perth` (Arch Linux)

**Configuration:**
- First-run creates `~/.config/perth/config.toml`
- Environment variables override config
- `znav config set` for runtime changes

### Infrastructure as Code

Not applicable for local CLI tool.

For shared Redis deployment:
```yaml
# docker-compose.yml (development)
services:
  redis:
    image: redis:6-alpine
    command: redis-server --appendonly yes
    volumes:
      - redis_data:/data
    ports:
      - "6379:6379"

volumes:
  redis_data:
```

---

## Requirements Traceability

### Functional Requirements Coverage

| FR ID | FR Name | Components | Implementation Notes |
|-------|---------|------------|---------------------|
| FR-001 | Manual Intent Logging | CLI, Orchestrator, State | `znav pane log` command |
| FR-002 | Intent History Retrieval | CLI, Orchestrator, State, Output | `znav pane history` command |
| FR-003 | Intent Entry Classification | CLI, Types | `--type` flag, IntentType enum |
| FR-004 | Artifact Tracking | CLI, State | `--artifacts` flag, artifacts hash |
| FR-005 | Multiple Output Formats | CLI, Output | `--format` flag, OutputFormatter |
| FR-006 | LLM Auto-Summarization | CLI, Orchestrator, LLM, Filter | `znav pane snapshot` command |
| FR-007 | Shell Hook Integration | External (zshrc/bashrc) | Documentation, example hooks |
| FR-008 | Secret Pattern Filtering | Filter | SecretFilter module |
| FR-009 | Local Model Fallback | LLM | LocalProvider implementation |
| FR-010 | Configurable Snapshot Triggers | Config | `intent_tracking.auto_snapshot_interval` |
| FR-011 | Context Recovery API | CLI, Output | `--format context` option |
| FR-012 | Bloodbank Event Publishing | Events | EventPublisher, milestone events |
| FR-013 | Agent Checkpoint Recording | CLI | `--source agent` flag |
| FR-014 | Session Resumption Context | CLI, Orchestrator | Show last_intent on pane nav |
| FR-015 | Semantic Search | (Phase 4) | Future: vector embeddings |
| FR-016 | Goal State Tracking | (Phase 4) | Future: goal subcommand |
| FR-017 | Pattern Recognition | (Phase 4) | Future: analytics module |
| FR-018 | Dashboard Visualization | Events | Bloodbank events → Holocene |
| FR-019 | Export to Markdown | Output | `--format markdown` option |
| FR-020 | Pane-First Navigation | CLI, Orchestrator, Zellij | v1.0 existing |
| FR-021 | Redis-Backed State | State | v1.0 existing |
| FR-022 | Auto-Focus via Position | Orchestrator, Zellij | v1.0 existing |
| FR-023 | Metadata Attachment | CLI, State | v1.0 existing `--meta` |
| FR-024 | Reconciliation | Orchestrator, State, Zellij | v1.0 existing `reconcile` |
| FR-025 | Tree Visualization | Orchestrator, Output | v1.0 existing `list` |

### Non-Functional Requirements Coverage

| NFR ID | NFR Name | Solution | Validation |
|--------|----------|----------|------------|
| NFR-001 | Command Latency <100ms | Connection pooling, async I/O | Benchmark suite, CI gate |
| NFR-002 | LLM Snapshot <3s | Streaming, timeout, circuit breaker | Latency monitoring |
| NFR-003 | History Query <100ms | Redis LRANGE, fixed depth | Benchmark suite |
| NFR-004 | Secret Filtering 100% | Fail-closed, pre-compiled regex | Fuzz testing, unit tests |
| NFR-005 | User Consent for LLM | Config flag, first-run prompt | Integration test |
| NFR-006 | Opt-In/Opt-Out Granularity | Hierarchical config | Config validation tests |
| NFR-007 | Local-Only Mode | NoOpProvider, offline Redis | Network isolation test |
| NFR-008 | State Persistence | Redis AOF/RDB | Restart tests |
| NFR-009 | Graceful LLM Failure | Circuit breaker, fallback | Failure injection tests |
| NFR-010 | No Shell Degradation | Async hooks, rate limiting | Prompt latency benchmark |
| NFR-011 | 100+ Panes Scalability | Redis efficiency, bounded queries | Load test with 100 panes |
| NFR-012 | 100 Entry History | LTRIM, fixed depth | Benchmark with full history |
| NFR-013 | <5MB Storage | Bounded history, TTL | Storage audit |
| NFR-014 | Modular Architecture | Clear module boundaries, traits | Code review, no circular deps |
| NFR-015 | Comprehensive Errors | anyhow, context chaining | Error message audit |
| NFR-016 | Backwards Compatibility | Additive schema, migration tool | Upgrade tests |
| NFR-017 | Human-Readable Output | OutputFormatter, colors | Manual review |
| NFR-018 | Machine-Readable JSON | serde_json, stable schema | JSON schema validation |
| NFR-019 | Help Documentation | Clap auto-help, README | Documentation completeness |
| NFR-020 | Zellij v0.39+ | Version check on startup | Multi-version CI |
| NFR-021 | Multi-Shell Support | Shell-agnostic hooks | Manual testing zsh/bash/fish |
| NFR-022 | Multi-LLM Provider | Provider trait, factory | Unit tests per provider |

---

## Trade-offs & Decision Log

### Decision 1: Continue with Rust

**Trade-off:**
- ✓ Gain: Performance, memory safety, existing codebase, single binary
- ✗ Lose: Slower iteration, contributor barrier, more verbose code

**Rationale:**
v1.0 already in Rust with working foundation. Migration cost would exceed benefits. Rust performance aligns with <100ms latency requirement.

---

### Decision 2: Redis as Sole Database

**Trade-off:**
- ✓ Gain: Speed, simplicity, already deployed, well-suited data model
- ✗ Lose: No complex queries, memory-bound, SPOF for intent data

**Rationale:**
Intent history is linear list per pane - perfect Redis fit. Complex queries (semantic search) deferred to Phase 4 with vector DB. Local Redis acceptable SPOF for personal tool.

---

### Decision 3: Multi-Provider LLM Abstraction

**Trade-off:**
- ✓ Gain: User choice, privacy options, vendor independence
- ✗ Lose: Implementation complexity, testing burden, abstraction cost

**Rationale:**
Privacy concerns (NFR-005, NFR-007) require local option. Different users prefer different providers. Abstraction via trait is idiomatic Rust, minimal runtime cost.

---

### Decision 4: CLI-First (No Web Service)

**Trade-off:**
- ✓ Gain: Simplicity, no deployment overhead, works offline
- ✗ Lose: No remote access, harder multi-user scenarios, no real-time features

**Rationale:**
Perth is terminal context tool - CLI fits naturally. Web service would add complexity without clear benefit. Remote features via ecosystem (Holocene dashboard via Bloodbank events).

---

### Decision 5: Bloodbank Events (Not Direct API)

**Trade-off:**
- ✓ Gain: Loose coupling, async delivery, ecosystem integration
- ✗ Lose: Event complexity, RabbitMQ dependency, eventual consistency

**Rationale:**
Events align with 33GOD event-driven architecture. Holocene doesn't need real-time guarantees. Optional dependency - Perth works without Bloodbank.

---

### Decision 6: Fail-Closed Secret Filtering

**Trade-off:**
- ✓ Gain: Security guarantee, no secret leakage
- ✗ Lose: Filter errors block LLM features, potential false positives

**Rationale:**
Secret leakage is catastrophic - fail-closed is only acceptable approach. False positives (over-filtering) annoying but safe. Users can customize patterns if needed.

---

## Open Issues & Risks

### Open Issues

| ID | Issue | Owner | Status |
|----|-------|-------|--------|
| ARCH-001 | Vector DB selection for Phase 4 semantic search | Engineering | Deferred to Phase 4 |
| ARCH-002 | Shell hook installation UX | Engineering | Design needed |
| ARCH-003 | Agent source validation (prevent spoofing) | Engineering + Jelmore | Needs Jelmore coordination |
| ARCH-004 | Holocene timeline widget design | Holocene team | Cross-team coordination |

### Risks

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| LLM costs exceed $5/month | Medium | Medium | Cost monitoring, local fallback, rate limiting |
| Secret filtering bypass | High | Low | Fail-closed design, fuzz testing, security audit |
| Redis data loss | Medium | Low | AOF persistence, backup recommendations |
| Zellij API breaking change | Medium | Low | Version check, CI against multiple versions |
| Low adoption | Medium | Medium | Alpha testing, user feedback loop |

---

## Assumptions & Constraints

### Assumptions

1. Users have Redis available locally or remotely
2. Users are willing to provide LLM API keys for auto-snapshot
3. Zellij CLI interface stable across minor versions
4. Shell history accessible via standard mechanisms ($HISTFILE)
5. 33GOD ecosystem components (Bloodbank, Jelmore) exist and function

### Constraints

1. Single-user local tool (no multi-tenancy in v2.0)
2. Redis-only persistence (no SQLite, Postgres options)
3. English-only summaries (LLM prompts not localized)
4. Linux/macOS only (no Windows support)

---

## Future Considerations

### Phase 3: Agent Integration Enhancements
- HMAC-signed agent checkpoints (anti-spoofing)
- Bulk history export for agent context
- Jelmore SDK for Perth operations

### Phase 4: Advanced Intelligence
- Vector database (Qdrant/Chroma) for semantic search
- Goal tracking with progress estimation
- Pattern recognition for workflow insights
- Holocene timeline widget

### v3.0 and Beyond
- Multi-user support with Redis ACLs
- Real-time sync for shared sessions
- IDE extensions (VS Code, JetBrains)
- Advanced analytics and reporting

---

## Approval & Sign-off

**Review Status:**
- [ ] Technical Lead
- [ ] Product Owner
- [ ] Security Review (secret filtering)
- [ ] DevOps Review (CI/CD, deployment)

---

## Revision History

| Version | Date | Author | Changes |
|---------|------|--------|---------|
| 1.0 | 2026-01-05 | delorenj | Initial architecture |
| 1.1 | 2026-01-06 | delorenj | Added Session Restoration Component (v2.1) |

---

## Next Steps

### Phase 4: Sprint Planning & Implementation

Run `/sprint-planning` to:
- Break 5 epics into detailed user stories
- Estimate story complexity (S/M/L/XL)
- Plan sprint iterations (Phase 1: 2-3 sprints, Phase 2: 3-4 sprints)
- Begin implementation following this architectural blueprint

**Key Implementation Principles:**
1. Follow module boundaries defined in this document
2. Implement LLMProvider trait before specific providers
3. Secret filtering must be fail-closed from day one
4. All commands must respect <100ms latency target
5. Event publishing is optional - graceful degradation required

**Implementation Priority:**
1. Phase 1: FR-001 through FR-005 (manual intent logging)
2. Phase 2: FR-006 through FR-010 (auto-snapshots)
3. Phase 3: FR-011 through FR-014 (agent integration)
4. Phase 4: FR-015 through FR-019 (advanced intelligence)

---

**This document was created using BMAD Method v6 - Phase 3 (Solutioning)**

*To continue: Run `/workflow-status` to see your progress and next recommended workflow.*

---

## Session Restoration Component (v2.1)

**Added:** 2026-01-06
**Status:** Proposed

This section defines the restoration component that provides session persistence beyond Zellij's built-in resurrection feature. The restoration module captures working directories, running commands, scroll positions, and git worktree context—enabling more robust session recovery.

### Architectural Drivers for Restoration

| ID | Requirement | Architectural Impact |
|----|-------------|---------------------|
| NFR-REST-001 | Snapshot latency <500ms | Parallel state capture, async I/O |
| NFR-REST-002 | 100% structure fidelity | Capture all tab/pane topology, positions |
| NFR-REST-003 | Storage efficiency <10KB delta | Incremental snapshots, compression |
| NFR-REST-004 | Graceful degradation | Continue with partial restore if CWD missing |
| NFR-REST-005 | Event integration | Publish snapshot/restore events to Bloodbank |
| NFR-REST-006 | Secret safety | Apply SecretFilter to command history in snapshots |

### Component: Restoration Module (`restoration.rs`)

**Purpose:** Capture and restore complete session state including working directories, running commands, and git context

**Responsibilities:**
- Capture session topology (tabs, panes, positions)
- Capture per-pane context (CWD, running command, scroll position)
- Store snapshots to Redis with TTL
- Restore session state from snapshot
- Support incremental snapshots (diff from last)
- Integrate with iMi worktree context (optional)

**Interfaces:**
```rust
pub struct RestorationManager {
    state: StateManager,
    zellij: ZellijDriver,
    events: Option<EventPublisher>,
}

impl RestorationManager {
    /// Capture current session state as named snapshot
    pub async fn snapshot(&self, name: &str) -> Result<SessionSnapshot>;

    /// Restore session from named snapshot
    pub async fn restore(&self, name: &str) -> Result<RestoreReport>;

    /// List available snapshots
    pub async fn list_snapshots(&self) -> Result<Vec<SnapshotMeta>>;

    /// Delete snapshot by name
    pub async fn delete_snapshot(&self, name: &str) -> Result<()>;

    /// Capture incremental delta from last snapshot
    pub async fn snapshot_incremental(&self, name: &str) -> Result<SessionSnapshot>;

    /// Get diff between two snapshots
    pub async fn diff(&self, a: &str, b: &str) -> Result<SnapshotDiff>;
}
```

**Dependencies:** StateManager, ZellijDriver, EventPublisher

**FRs Addressed:** New FR-026 through FR-030 (see below)

---

### Restoration Data Model

**Core Entities:**

```
┌─────────────────────────────────────────────────────────────────┐
│                       SessionSnapshot                            │
│  name: String (user-provided identifier)                        │
│  created_at: DateTime<Utc>                                      │
│  session_name: String (Zellij session)                          │
│  is_incremental: bool                                           │
│  parent_snapshot: Option<String>                                │
│  tabs: Vec<TabSnapshot>                                         │
│  metadata: HashMap<String, String>                              │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                         TabSnapshot                              │
│  name: String                                                   │
│  position: u32                                                  │
│  is_active: bool                                                │
│  panes: Vec<PaneSnapshot>                                       │
│  layout_hint: LayoutHint (tiled, stacked, custom)              │
└─────────────────────────────────────────────────────────────────┘
                                │
                                ▼
┌─────────────────────────────────────────────────────────────────┐
│                         PaneSnapshot                             │
│  name: Option<String>                                           │
│  position: PanePosition (row, col, width, height)              │
│  cwd: PathBuf                                                   │
│  running_command: Option<String>                                │
│  scroll_offset: u32                                             │
│  is_focused: bool                                               │
│  env_vars: Option<HashMap<String, String>>                     │
│  git_branch: Option<String>                                     │
│  imi_worktree: Option<String> (iMi integration)                │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                        RestoreReport                             │
│  snapshot_name: String                                          │
│  tabs_restored: u32                                             │
│  panes_restored: u32                                            │
│  warnings: Vec<RestoreWarning>                                  │
│  errors: Vec<RestoreError>                                      │
│  duration_ms: u64                                               │
└─────────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────┐
│                        RestoreWarning                            │
│  pane: String                                                   │
│  issue: WarningType (CwdMissing, CommandUnavailable, etc.)     │
│  fallback: String (what was done instead)                      │
└─────────────────────────────────────────────────────────────────┘
```

### Redis Schema for Restoration

**Keyspace: `perth:snapshots:*`**

```
# Snapshot metadata (Hash)
perth:snapshots:{session}:{name}
  Fields:
    created_at   -> String (ISO 8601)
    session      -> String (Zellij session name)
    is_incremental -> bool
    parent       -> Option<String> (parent snapshot name)
    tab_count    -> u32
    pane_count   -> u32
    size_bytes   -> u32

# Snapshot data (String, JSON-serialized SessionSnapshot)
perth:snapshots:{session}:{name}:data
  Value: JSON SessionSnapshot (gzip compressed if >1KB)
  TTL: 30 days (configurable via restoration.snapshot_ttl_days)

# Snapshot index (Sorted Set, by creation time)
perth:snapshots:{session}:index
  Members: snapshot names
  Scores: Unix timestamp of creation

# Latest snapshot pointer (String)
perth:snapshots:{session}:latest
  Value: name of most recent snapshot
```

**Storage Estimates:**
| Entity | Size | Count | Total |
|--------|------|-------|-------|
| Snapshot metadata | 200 bytes | 50 snapshots | 10 KB |
| Snapshot data (full) | 2-5 KB | 10 full | 50 KB |
| Snapshot data (incremental) | 200-500 bytes | 40 incremental | 20 KB |
| **Total per session** | | | **~80 KB** |

---

### Restoration CLI Commands

```bash
# Create named snapshot
znav snapshot create <name> [--incremental]

# List available snapshots
znav snapshot list [--session <session>] [--format text|json]

# Restore from snapshot
znav snapshot restore <name> [--dry-run]

# Delete snapshot
znav snapshot delete <name>

# Show snapshot diff
znav snapshot diff <snapshot-a> <snapshot-b>

# Auto-snapshot on interval (background daemon)
znav snapshot daemon --interval 10m
```

---

### Restoration Orchestrator Integration

The `Orchestrator` struct gains new methods for restoration:

```rust
impl Orchestrator {
    // ... existing methods ...

    // Restoration (v2.1)
    pub async fn create_snapshot(&mut self, name: &str, incremental: bool) -> Result<SessionSnapshot>;
    pub async fn restore_snapshot(&mut self, name: &str) -> Result<RestoreReport>;
    pub async fn list_snapshots(&mut self) -> Result<Vec<SnapshotMeta>>;
    pub async fn delete_snapshot(&mut self, name: &str) -> Result<()>;
}
```

---

### Restoration NFR Coverage

#### NFR-REST-001: Snapshot Latency <500ms

**Requirement:** Full session snapshot completes in <500ms

**Architecture Solution:**
- **Parallel pane capture** - spawn async tasks per pane
- **Cached Zellij layout** - reuse layout dump across panes
- **Streaming Redis writes** - pipeline multiple HSET calls

**Implementation Notes:**
```rust
pub async fn snapshot(&self, name: &str) -> Result<SessionSnapshot> {
    let layout = self.zellij.dump_layout()?;  // Single call

    // Parallel pane capture
    let pane_futures: Vec<_> = layout.panes.iter()
        .map(|p| self.capture_pane_state(p))
        .collect();

    let pane_snapshots = futures::future::join_all(pane_futures).await;

    // Pipeline Redis writes
    let mut pipe = redis::pipe();
    // ... batch operations
    pipe.query_async(&mut self.state.conn).await?;
}
```

**Validation:**
- Benchmark: 50-pane session snapshot <500ms
- CI gate: p95 < 500ms

---

#### NFR-REST-002: 100% Structure Fidelity

**Requirement:** Restored session matches original tab/pane topology exactly

**Architecture Solution:**
- Capture tab positions, not just names
- Capture pane geometry (row, col, width%, height%)
- Store focus state per tab and pane
- Preserve layout type (tiled, stacked, etc.)

**Implementation Notes:**
- Use `zellij action dump-layout` for structure
- Parse layout KDL for pane positions
- Store as normalized coordinates (percentages)

**Validation:**
- Integration test: snapshot → restore → dump-layout diff = empty
- Visual comparison in CI (screenshot diff)

---

#### NFR-REST-003: Storage Efficiency <10KB Delta

**Requirement:** Incremental snapshots average <10KB

**Architecture Solution:**
- **Structural diffing** - only store changed panes
- **Reference encoding** - `"cwd": "@parent"` for unchanged fields
- **gzip compression** - for snapshots >1KB
- **TTL cleanup** - auto-expire old snapshots

**Implementation Notes:**
```rust
pub async fn snapshot_incremental(&self, name: &str) -> Result<SessionSnapshot> {
    let parent = self.get_latest_snapshot()?;
    let current = self.capture_current_state()?;

    // Compute diff
    let diff = self.diff_snapshots(&parent, &current);

    SessionSnapshot {
        is_incremental: true,
        parent_snapshot: Some(parent.name),
        tabs: diff.changed_tabs,  // Only changed
        // ...
    }
}
```

**Validation:**
- Measure average incremental size across test sessions
- Target: <500 bytes for typical "moved to different pane" change

---

#### NFR-REST-004: Graceful Degradation

**Requirement:** Restoration continues with partial success if some elements can't be restored

**Architecture Solution:**
- **Warning accumulation** - collect issues, don't fail fast
- **Fallback behaviors:**
  - CWD missing → use home directory
  - Command unavailable → skip command execution
  - Git branch missing → warn and continue
- **Restore report** - detailed success/warning/error breakdown

**Implementation Notes:**
```rust
impl RestorationManager {
    async fn restore_pane(&self, snapshot: &PaneSnapshot) -> PaneRestoreResult {
        let mut warnings = Vec::new();

        // Attempt CWD restoration
        if !snapshot.cwd.exists() {
            warnings.push(RestoreWarning {
                pane: snapshot.name.clone(),
                issue: WarningType::CwdMissing,
                fallback: "Using home directory".into(),
            });
            // Use fallback CWD
        }

        // Continue with other restoration...
        PaneRestoreResult { warnings, success: true }
    }
}
```

**Validation:**
- Test: restore with deleted directories succeeds with warnings
- Test: RestoreReport accurately reflects partial success

---

#### NFR-REST-005: Event Integration

**Requirement:** Publish snapshot/restore events to Bloodbank

**Architecture Solution:**
- Extend `PerthEvent` enum with restoration events
- Fire-and-forget publishing (non-blocking)
- Include snapshot metadata in events

**Events:**
```rust
pub enum PerthEvent {
    // ... existing events ...

    SnapshotCreated {
        session: String,
        name: String,
        tab_count: u32,
        pane_count: u32,
        is_incremental: bool,
        timestamp: DateTime<Utc>,
    },

    SnapshotRestored {
        session: String,
        name: String,
        tabs_restored: u32,
        panes_restored: u32,
        warnings: u32,
        errors: u32,
        duration_ms: u64,
        timestamp: DateTime<Utc>,
    },
}
```

**Validation:**
- Integration test: verify events published on snapshot/restore
- Test: restoration succeeds if Bloodbank unavailable

---

#### NFR-REST-006: Secret Safety

**Requirement:** Command history in snapshots filtered for secrets

**Architecture Solution:**
- Apply existing `SecretFilter` to `PaneSnapshot.running_command`
- Filter any captured environment variables
- Log filtered content (pattern names only, not values)

**Implementation Notes:**
```rust
impl RestorationManager {
    fn capture_pane_state(&self, pane: &LayoutPane) -> PaneSnapshot {
        let running_command = self.zellij.get_running_command(pane);

        // Apply secret filter
        let filtered_command = running_command.map(|cmd| {
            self.filter.filter(&cmd).filtered
        });

        PaneSnapshot {
            running_command: filtered_command,
            // ...
        }
    }
}
```

**Validation:**
- Unit test: secrets redacted in snapshot
- Fuzz test: no secrets in stored snapshot data

---

### New Functional Requirements (FR-026 through FR-030)

| FR ID | FR Name | Description |
|-------|---------|-------------|
| FR-026 | Session Snapshot Creation | User can create named snapshots of current session state |
| FR-027 | Session Restoration | User can restore session from named snapshot |
| FR-028 | Snapshot Listing | User can list available snapshots with metadata |
| FR-029 | Incremental Snapshots | System supports delta snapshots from previous state |
| FR-030 | Snapshot Auto-Daemon | Background process can auto-snapshot on interval |

---

### Module Structure Update

```
src/
  ├── main.rs
  ├── cli.rs                  # Add Snapshot subcommand
  ├── orchestrator.rs         # Add restoration methods
  ├── state.rs
  ├── zellij.rs               # Add layout parsing, command capture
  ├── llm/
  ├── filter.rs
  ├── events.rs               # Add SnapshotCreated, SnapshotRestored
  ├── output.rs               # Add snapshot formatters
  ├── types.rs                # Add SessionSnapshot, etc.
  ├── restoration/            # NEW MODULE
  │   ├── mod.rs              # RestorationManager
  │   ├── capture.rs          # State capture logic
  │   ├── restore.rs          # Restoration logic
  │   ├── diff.rs             # Incremental diffing
  │   └── daemon.rs           # Auto-snapshot daemon
  └── error.rs
```

---

### Trade-offs: Restoration Component

#### Decision 7: Restoration in Core vs. Separate Plugin

**Trade-off:**
- ✓ Gain: Unified codebase, shared Redis/Zellij drivers, consistent UX
- ✗ Lose: Larger binary, tighter coupling

**Rationale:**
Restoration is core to Perth's value proposition—it extends pane context with session durability. Sharing StateManager and ZellijDriver avoids duplication. As discussed, this is a module within Zellij Driver, not a separate plugin.

---

#### Decision 8: Incremental vs. Full Snapshots Only

**Trade-off:**
- ✓ Gain: Storage efficiency (10x reduction), faster writes
- ✗ Lose: Implementation complexity, restoration must chain parents

**Rationale:**
Users may snapshot frequently (every 10 minutes). Full snapshots would consume ~5KB each, adding 720KB/day. Incremental reduces to ~500 bytes average. Complexity acceptable given storage savings.

---

#### Decision 9: Daemon vs. Shell Hook for Auto-Snapshot

**Trade-off:**
- ✓ Gain (Daemon): Reliable interval, no shell hook complexity
- ✗ Lose (Daemon): Another process, resource usage

**Rationale:**
Shell hooks already have latency constraints (NFR-010: <10ms). A separate daemon with `znav snapshot daemon` allows configurable intervals without shell overhead. Can be managed via systemd user unit.

---

### Risks: Restoration Component

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Layout dump format changes | High | Low | Version check, fallback parser |
| Stale snapshot after refactoring | Medium | Medium | Warnings on restore, suggest fresh snapshot |
| Storage bloat from many snapshots | Medium | Low | TTL cleanup, snapshot count limits |
| Restore fails on different Zellij version | Medium | Medium | Store Zellij version, warn on mismatch |

---

## Appendix A: Technology Evaluation Matrix

### LLM Provider Comparison

| Criteria | Claude | GPT-4o-mini | Ollama (Local) |
|----------|--------|-------------|----------------|
| Summary Quality | Excellent | Very Good | Good |
| Latency | 1-3s | 1-2s | 2-5s |
| Cost per snapshot | ~$0.003 | ~$0.002 | $0 |
| Privacy | Cloud | Cloud | Local |
| Availability | 99.9% | 99.9% | Varies |
| **Recommendation** | Primary | Alternative | Privacy mode |

### Database Alternatives (Evaluated)

| Option | Pros | Cons | Decision |
|--------|------|------|----------|
| Redis | Fast, simple, deployed | Memory-bound, no complex queries | **Selected** |
| SQLite | Embedded, SQL queries | Slower for simple lookups | Rejected |
| PostgreSQL | Full SQL, scalable | Overkill, deployment overhead | Rejected |
| File-based | Zero dependencies | Slow, no queries, corruption risk | Rejected |

---

## Appendix B: Capacity Planning

### Storage Estimates

| Entity | Size | Count | Total |
|--------|------|-------|-------|
| Pane metadata | 500 bytes | 100 panes | 50 KB |
| Intent entry | 500 bytes | 100 per pane | 5 MB |
| Artifacts | 100 bytes | 10 per pane | 100 KB |
| **Total (100 panes)** | | | **~5 MB** |

### Performance Estimates

| Operation | Target | Expected |
|-----------|--------|----------|
| `znav pane log` | <50ms | 5-10ms |
| `znav pane history` | <100ms | 20-50ms |
| `znav pane snapshot` | <3s | 1-2s |
| `znav list` (100 panes) | <200ms | 50-100ms |

---

## Appendix C: Cost Estimation

### LLM Costs (Per User Per Month)

| Usage Level | Snapshots/Day | Claude Cost | GPT Cost | Local Cost |
|-------------|---------------|-------------|----------|------------|
| Light | 5 | $0.45 | $0.30 | $0 |
| Normal | 20 | $1.80 | $1.20 | $0 |
| Heavy | 50 | $4.50 | $3.00 | $0 |
| Power | 100 | $9.00 | $6.00 | $0 |

**Recommendation:** Default to 10-20 snapshots/day for <$2/month with Claude.

### Infrastructure Costs

| Component | Self-Hosted | Managed |
|-----------|-------------|---------|
| Redis | $0 (local) | $15/mo (Redis Cloud) |
| Bloodbank | Internal | Internal |

**Recommendation:** Self-hosted Redis for personal use, managed for team deployments.
