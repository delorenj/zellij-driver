# Sprint Plan: Perth

**Date:** 2026-01-05
**Scrum Master:** delorenj
**Project Level:** 3
**Total Stories:** 38
**Total Points:** 143
**Planned Sprints:** 6

---

## Executive Summary

This sprint plan covers Perth v2.0 development, building on the implemented v1.0 navigation primitives (EPIC-001) to add cognitive context management features. The plan is organized into 4 implementation phases across 6 sprints, prioritizing manual intent logging (Phase 1) as the MVP foundation before progressing to automated capture, agent integration, and advanced intelligence features.

**Key Metrics:**
- Total Stories: 38 (v2.0 scope)
- Total Points: 143 points
- Sprints: 6 (2 weeks each)
- Team: Solo developer (senior)
- Velocity Target: 25-30 points/sprint
- Target Completion: ~12 weeks

**Implementation Phases:**
- Phase 1 (Sprint 1-2): Manual Intent Logging - EPIC-002
- Phase 2 (Sprint 3-4): Automated Context Capture - EPIC-003
- Phase 3 (Sprint 5): Agent Integration - EPIC-004
- Phase 4 (Sprint 6+): Advanced Intelligence - EPIC-005 (partial)

---

## Story Inventory

### EPIC-002: Manual Intent Logging (Phase 1)

---

#### STORY-001: IntentEntry Data Model

**Epic:** EPIC-002
**Priority:** Must Have
**Points:** 3

**User Story:**
As a developer working on Perth,
I want a well-defined IntentEntry data type
So that intent history has consistent structure across all features.

**Acceptance Criteria:**
- [ ] `IntentEntry` struct defined in `types.rs` with all fields
- [ ] Implements `Serialize`/`Deserialize` for JSON and Redis
- [ ] `IntentType` enum: Milestone, Checkpoint, Exploration
- [ ] `IntentSource` enum: Manual, Automated, Agent
- [ ] Timestamp uses `chrono::DateTime<Utc>`
- [ ] Unit tests for serialization round-trip

**Technical Notes:**
- Extend existing `types.rs` module
- Fields: id, timestamp, summary, entry_type, artifacts, commands_run, goal_delta, source
- UUID for entry id generation

**Dependencies:** None (foundational)

---

#### STORY-002: Redis Intent History Schema

**Epic:** EPIC-002
**Priority:** Must Have
**Points:** 5

**User Story:**
As a developer,
I want intent history stored in Redis as ordered lists
So that history persists across sessions and supports efficient queries.

**Acceptance Criteria:**
- [ ] `perth:pane:{name}:history` key stores JSON list (newest first)
- [ ] `log_intent()` method in StateManager uses LPUSH
- [ ] `get_history()` method uses LRANGE with limit support
- [ ] `last_intent` field updated on pane hash on each log
- [ ] LTRIM maintains max 100 entries per pane
- [ ] Integration tests with real Redis

**Technical Notes:**
- Extend `state.rs` with new async methods
- Use redis-rs LIST operations
- Consider TTL for old entries (configurable, default 90 days)

**Dependencies:** STORY-001

---

#### STORY-003: CLI Log Command

**Epic:** EPIC-002
**Priority:** Must Have
**Points:** 5

**User Story:**
As a developer,
I want to run `znav pane log <name> "<summary>"`
So that I can record what I accomplished in a work session.

**Acceptance Criteria:**
- [ ] Command parses pane name and summary from args
- [ ] Validates pane exists (or creates implicit reference)
- [ ] Creates IntentEntry with timestamp, source=Manual
- [ ] Stores entry via StateManager.log_intent()
- [ ] Prints confirmation with entry ID
- [ ] Completes in <50ms (p95)

**Technical Notes:**
- Add `Log` variant to `Command` enum in `cli.rs`
- Add `log_intent()` to Orchestrator
- Summary max 500 chars (validate)

**Dependencies:** STORY-001, STORY-002

---

#### STORY-004: Entry Type Classification Flag

**Epic:** EPIC-002
**Priority:** Must Have
**Points:** 2

**User Story:**
As a developer,
I want to use `--type milestone|checkpoint|exploration`
So that I can distinguish major progress from exploratory work.

**Acceptance Criteria:**
- [ ] `--type` flag accepts: milestone, checkpoint, exploration
- [ ] Default type is "checkpoint" if not specified
- [ ] Invalid type shows error with valid options
- [ ] Type stored in IntentEntry.entry_type
- [ ] Help text explains each type

**Technical Notes:**
- Extend LogArgs in `cli.rs`
- Clap value_enum for IntentType

**Dependencies:** STORY-003

---

#### STORY-005: Artifact Tracking Flag

**Epic:** EPIC-002
**Priority:** Should Have
**Points:** 3

**User Story:**
As a developer,
I want to use `--artifacts file1.py file2.js`
So that I can associate code changes with my intent entries.

**Acceptance Criteria:**
- [ ] `--artifacts` flag accepts multiple file paths
- [ ] Relative paths resolved to absolute
- [ ] Non-existent files allowed (for deleted files)
- [ ] Artifacts stored in IntentEntry.artifacts Vec
- [ ] Artifacts shown in history output

**Technical Notes:**
- Use std::fs::canonicalize for path resolution
- Store as strings (not PathBuf) for JSON serialization

**Dependencies:** STORY-003

---

#### STORY-006: CLI History Command

**Epic:** EPIC-002
**Priority:** Must Have
**Points:** 5

**User Story:**
As a developer,
I want to run `znav pane history <name>`
So that I can see my previous work on a pane.

**Acceptance Criteria:**
- [ ] Command retrieves all history for named pane
- [ ] Default output is human-readable text format
- [ ] Shows timestamp, type, summary for each entry
- [ ] Empty history shows friendly message
- [ ] Completes in <100ms (p95)

**Technical Notes:**
- Add `History` variant to `Command` enum
- Add `get_history()` to Orchestrator
- Create OutputFormatter for history display

**Dependencies:** STORY-002

---

#### STORY-007: History Limit Flag

**Epic:** EPIC-002
**Priority:** Must Have
**Points:** 2

**User Story:**
As a developer,
I want to use `--last N` flag
So that I can see only recent entries without scrolling.

**Acceptance Criteria:**
- [ ] `--last N` limits output to N most recent entries
- [ ] Default (no flag) shows all entries (up to 100)
- [ ] Invalid N (negative, non-numeric) shows error
- [ ] Works with all output formats

**Technical Notes:**
- Pass limit to StateManager.get_history()
- LRANGE 0 (N-1) in Redis

**Dependencies:** STORY-006

---

#### STORY-008: JSON Output Format

**Epic:** EPIC-002
**Priority:** Must Have
**Points:** 3

**User Story:**
As a developer,
I want to use `--format json`
So that I can build scripts and integrations with history data.

**Acceptance Criteria:**
- [ ] `--format json` outputs valid JSON array
- [ ] JSON includes all IntentEntry fields
- [ ] Output parseable by jq, Python json.loads()
- [ ] Schema version field included in output
- [ ] Works with history and other commands

**Technical Notes:**
- OutputFormatter handles format selection
- Use serde_json::to_string_pretty for pretty output
- Consider `--format json-compact` for single-line

**Dependencies:** STORY-006

---

#### STORY-009: Human-Readable Output Formatting

**Epic:** EPIC-002
**Priority:** Must Have
**Points:** 5

**User Story:**
As a developer,
I want nicely formatted, color-coded history output
So that I can quickly scan my work history.

**Acceptance Criteria:**
- [ ] Timestamps shown as relative ("2 hours ago")
- [ ] Milestones highlighted with distinct color
- [ ] Entry types shown with icons or labels
- [ ] Long summaries wrap gracefully
- [ ] Respects NO_COLOR environment variable
- [ ] Works with terminal width detection

**Technical Notes:**
- Use `colored` crate for ANSI colors
- Use `chrono-humanize` for relative times
- Detect terminal width with `terminal_size` crate

**Dependencies:** STORY-006

---

#### STORY-010: Help Documentation for Intent Commands

**Epic:** EPIC-002
**Priority:** Must Have
**Points:** 2

**User Story:**
As a developer,
I want comprehensive `--help` for log and history commands
So that I can learn the features without reading docs.

**Acceptance Criteria:**
- [ ] `znav pane log --help` shows all options with examples
- [ ] `znav pane history --help` shows all options with examples
- [ ] Examples show common use cases
- [ ] Help mentions related commands

**Technical Notes:**
- Clap `#[command(about = "...")]` and `#[arg(help = "...")]`
- Add example section with `#[command(after_help = "...")]`

**Dependencies:** STORY-003, STORY-006

---

### EPIC-003: Automated Context Capture (Phase 2)

---

#### STORY-011: LLM Provider Trait

**Epic:** EPIC-003
**Priority:** Must Have
**Points:** 5

**User Story:**
As a developer working on Perth,
I want an abstracted LLM provider interface
So that I can swap providers without changing business logic.

**Acceptance Criteria:**
- [ ] `LLMProvider` trait defined with `summarize()` method
- [ ] Trait is async and Send + Sync
- [ ] SessionContext struct captures shell history, git diff, files
- [ ] `NoOpProvider` returns error when LLM disabled
- [ ] Factory function creates provider from config

**Technical Notes:**
- Create `llm/mod.rs` with trait definition
- Use `#[async_trait]` macro
- Box<dyn LLMProvider> for runtime polymorphism

**Dependencies:** None (foundational for Phase 2)

---

#### STORY-012: Anthropic Claude Provider

**Epic:** EPIC-003
**Priority:** Must Have
**Points:** 5

**User Story:**
As a developer,
I want Perth to use Claude API for summarization
So that I get high-quality intent summaries.

**Acceptance Criteria:**
- [ ] `AnthropicProvider` implements `LLMProvider` trait
- [ ] Uses reqwest for HTTPS calls to Claude API
- [ ] API key from config or ANTHROPIC_API_KEY env var
- [ ] Streaming response support for <3s latency
- [ ] Proper error handling for rate limits, timeouts
- [ ] Unit tests with mock HTTP responses

**Technical Notes:**
- Claude messages API endpoint
- Model: claude-3-5-sonnet-20241022 (configurable)
- System prompt for intent summarization

**Dependencies:** STORY-011

---

#### STORY-013: OpenAI GPT Provider

**Epic:** EPIC-003
**Priority:** Should Have
**Points:** 3

**User Story:**
As a developer,
I want GPT as an alternative LLM provider
So that I have options based on cost and preference.

**Acceptance Criteria:**
- [ ] `OpenAIProvider` implements `LLMProvider` trait
- [ ] Uses reqwest for HTTPS calls to OpenAI API
- [ ] API key from config or OPENAI_API_KEY env var
- [ ] Model: gpt-4o-mini (configurable)
- [ ] Error handling consistent with Anthropic provider

**Technical Notes:**
- OpenAI chat completions API
- Similar structure to Anthropic provider

**Dependencies:** STORY-011

---

#### STORY-014: Local Ollama Provider

**Epic:** EPIC-003
**Priority:** Should Have
**Points:** 3

**User Story:**
As a developer,
I want to use a local LLM via Ollama
So that my context never leaves my machine.

**Acceptance Criteria:**
- [ ] `LocalProvider` implements `LLMProvider` trait
- [ ] Connects to localhost Ollama endpoint
- [ ] Model configurable (default: llama3)
- [ ] Graceful error if Ollama not running
- [ ] Lower quality acceptable for privacy trade-off

**Technical Notes:**
- Ollama API at http://localhost:11434
- /api/generate endpoint

**Dependencies:** STORY-011

---

#### STORY-015: Secret Filter Module

**Epic:** EPIC-003
**Priority:** Must Have
**Points:** 5

**User Story:**
As a developer,
I want secrets automatically filtered before LLM submission
So that I don't accidentally leak credentials.

**Acceptance Criteria:**
- [ ] `SecretFilter` struct with configurable patterns
- [ ] Default patterns: password=, token=, key=, secret=, AWS_*
- [ ] Patterns load from config file
- [ ] `filter()` method returns sanitized string
- [ ] Fail-closed: filter errors abort LLM call
- [ ] Audit log shows redaction count (not content)

**Technical Notes:**
- Create `filter.rs` module
- Pre-compile regex at startup
- Use regex crate with pattern validation

**Dependencies:** None (foundational for safety)

---

#### STORY-016: Context Collector

**Epic:** EPIC-003
**Priority:** Must Have
**Points:** 5

**User Story:**
As a developer,
I want Perth to collect shell history and git diff
So that the LLM has context for summarization.

**Acceptance Criteria:**
- [ ] Collects last 20 commands from $HISTFILE
- [ ] Runs `git diff --stat` if in git repo
- [ ] Lists recently modified files (last 30 min)
- [ ] Packages into SessionContext struct
- [ ] Applies secret filter before returning
- [ ] Works with zsh, bash, fish history formats

**Technical Notes:**
- Shell history parsing varies by shell
- Use `std::process::Command` for git
- File modification check via metadata.modified()

**Dependencies:** STORY-015

---

#### STORY-017: CLI Snapshot Command

**Epic:** EPIC-003
**Priority:** Must Have
**Points:** 5

**User Story:**
As a developer,
I want to run `znav pane snapshot <name>`
So that Perth auto-generates an intent summary from my recent work.

**Acceptance Criteria:**
- [ ] Command collects context via ContextCollector
- [ ] Filters secrets before LLM submission
- [ ] Calls LLM provider to generate summary
- [ ] Stores result as IntentEntry with source=Automated
- [ ] Shows generated summary to user
- [ ] Completes in <3s (p95)

**Technical Notes:**
- Add `Snapshot` variant to Command enum
- Add `snapshot()` to Orchestrator
- Handle LLM timeout with circuit breaker

**Dependencies:** STORY-011, STORY-015, STORY-016

---

#### STORY-018: User Consent Flow

**Epic:** EPIC-003
**Priority:** Must Have
**Points:** 3

**User Story:**
As a developer,
I want Perth to request consent before sending data to LLM
So that I control what leaves my machine.

**Acceptance Criteria:**
- [ ] First `snapshot` command prompts for consent
- [ ] Explains what data is sent (shell history, git diff)
- [ ] Consent stored in config file
- [ ] `znav config consent --grant` grants consent
- [ ] `znav config consent --revoke` revokes consent
- [ ] Snapshot fails with helpful message if no consent

**Technical Notes:**
- `privacy.consent_given` in config
- Interactive prompt via stdin/stdout
- Non-interactive mode respects config silently

**Dependencies:** STORY-017

---

#### STORY-019: Circuit Breaker for LLM

**Epic:** EPIC-003
**Priority:** Must Have
**Points:** 3

**User Story:**
As a developer,
I want Perth to handle LLM failures gracefully
So that snapshot issues don't disrupt my workflow.

**Acceptance Criteria:**
- [ ] Circuit breaker opens after 3 consecutive failures
- [ ] Open circuit returns immediate error (no API call)
- [ ] Circuit half-opens after 5 minute cooldown
- [ ] Single success closes circuit
- [ ] Failure message suggests manual logging

**Technical Notes:**
- Implement CircuitBreaker struct
- Use AtomicU32 for thread-safe counters
- Store state in memory (resets on process restart)

**Dependencies:** STORY-017

---

#### STORY-020: Config for LLM Settings

**Epic:** EPIC-003
**Priority:** Must Have
**Points:** 3

**User Story:**
As a developer,
I want to configure LLM provider and settings
So that I can choose my preferred provider and model.

**Acceptance Criteria:**
- [ ] `[llm]` section in config.toml
- [ ] `provider`: anthropic, openai, local, none
- [ ] `model`: provider-specific model name
- [ ] `timeout_secs`: API timeout (default 10)
- [ ] API keys from config or env vars
- [ ] Config validation on load

**Technical Notes:**
- Extend Config struct in `config.rs`
- Serde for TOML parsing
- Env var override pattern

**Dependencies:** None

---

#### STORY-021: Shell Hook Documentation

**Epic:** EPIC-003
**Priority:** Should Have
**Points:** 2

**User Story:**
As a developer,
I want shell hook installation instructions
So that I can enable automatic snapshots.

**Acceptance Criteria:**
- [ ] Zsh precmd hook documented
- [ ] Bash PROMPT_COMMAND hook documented
- [ ] Fish event hook documented
- [ ] Rate limiting explained (1 per 10 commands)
- [ ] Troubleshooting section included

**Technical Notes:**
- Create docs/shell-hooks.md
- Include copy-paste snippets
- Explain PERTH_ENABLED env var

**Dependencies:** STORY-017

---

### EPIC-004: Agent Integration (Phase 3)

---

#### STORY-022: Agent Source Flag

**Epic:** EPIC-004
**Priority:** Must Have
**Points:** 2

**User Story:**
As a Jelmore agent,
I want to use `--source agent` when logging
So that my checkpoints are distinguishable from human entries.

**Acceptance Criteria:**
- [ ] `--source manual|agent` flag on log command
- [ ] Default source is "manual"
- [ ] Agent entries visually distinct in history output
- [ ] Source field queryable for filtering

**Technical Notes:**
- Extend LogArgs with source option
- IntentSource enum already exists

**Dependencies:** STORY-003

---

#### STORY-023: Context Output Format

**Epic:** EPIC-004
**Priority:** Must Have
**Points:** 3

**User Story:**
As a Jelmore agent,
I want `--format context` for optimized prompt injection
So that I can efficiently resume work with history context.

**Acceptance Criteria:**
- [ ] `--format context` produces LLM-friendly narrative
- [ ] Output limited to ~1000 tokens
- [ ] Includes chronological summary of work
- [ ] Highlights last checkpoint and current state
- [ ] Suggests next steps based on history

**Technical Notes:**
- Add Context variant to OutputFormat enum
- Template-based formatting
- Token estimation via word count

**Dependencies:** STORY-008

---

#### STORY-024: Session Resumption Display

**Epic:** EPIC-004
**Priority:** Should Have
**Points:** 3

**User Story:**
As a developer,
I want to see the last intent when navigating to a pane
So that I immediately remember what I was working on.

**Acceptance Criteria:**
- [ ] `znav pane <name>` shows last_intent if exists
- [ ] Display is single line, non-intrusive
- [ ] Includes relative timestamp
- [ ] Configurable via `show_last_intent` setting
- [ ] Skipped if no history exists

**Technical Notes:**
- Modify open_pane() in Orchestrator
- Read last_intent from pane hash
- Color-coded output

**Dependencies:** STORY-002

---

#### STORY-025: Bloodbank Event Publisher

**Epic:** EPIC-004
**Priority:** Should Have
**Points:** 5

**User Story:**
As a Yi orchestrator,
I want milestone events published to Bloodbank
So that I can track distributed agent progress in real-time.

**Acceptance Criteria:**
- [ ] `EventPublisher` connects to RabbitMQ
- [ ] Publishes on milestone-type entries
- [ ] Event includes pane, summary, timestamp, artifacts
- [ ] Async fire-and-forget (non-blocking)
- [ ] Graceful degradation if Bloodbank unavailable

**Technical Notes:**
- Create `events.rs` module
- Use lapin crate for RabbitMQ
- Exchange: perth.events, routing key: perth.milestone.recorded

**Dependencies:** STORY-003

---

#### STORY-026: Bloodbank Config

**Epic:** EPIC-004
**Priority:** Should Have
**Points:** 2

**User Story:**
As a developer,
I want to configure Bloodbank connection
So that events publish to my infrastructure.

**Acceptance Criteria:**
- [ ] `bloodbank_url` in config (optional)
- [ ] If not set, event publishing disabled
- [ ] Connection retry on temporary failure
- [ ] Clear error message on misconfiguration

**Technical Notes:**
- Extend Config with optional bloodbank_url
- EventPublisher checks for None before connecting

**Dependencies:** STORY-025

---

#### STORY-027: History Type Filter

**Epic:** EPIC-004
**Priority:** Should Have
**Points:** 2

**User Story:**
As a Jelmore agent,
I want to filter history by type (milestones only)
So that I can focus on major progress, not checkpoints.

**Acceptance Criteria:**
- [ ] `--type milestone` filters to milestones only
- [ ] Works with all output formats
- [ ] Can combine with --last N
- [ ] No filter returns all types (default)

**Technical Notes:**
- Client-side filtering after Redis fetch
- Consider Redis SCAN with pattern for efficiency

**Dependencies:** STORY-006

---

### EPIC-005: Advanced Intelligence (Phase 4)

---

#### STORY-028: Markdown Export

**Epic:** EPIC-005
**Priority:** Should Have
**Points:** 3

**User Story:**
As a developer,
I want to export history to Markdown
So that I can journal my progress in Obsidian.

**Acceptance Criteria:**
- [ ] `--format markdown` outputs valid Markdown
- [ ] YAML frontmatter with pane metadata
- [ ] Entries as bullet list with timestamps
- [ ] Artifacts as file links
- [ ] Obsidian-compatible format

**Technical Notes:**
- Add Markdown variant to OutputFormat
- Template-based rendering

**Dependencies:** STORY-008

---

#### STORY-029: Config Show Command

**Epic:** EPIC-005
**Priority:** Should Have
**Points:** 2

**User Story:**
As a developer,
I want to view my current configuration
So that I can verify settings are correct.

**Acceptance Criteria:**
- [ ] `znav config show` displays all settings
- [ ] Sensitive values (API keys) masked
- [ ] Shows defaults and overrides
- [ ] Indicates config file location

**Technical Notes:**
- Add Config subcommand to CLI
- Mask with `***` for sensitive fields

**Dependencies:** None

---

#### STORY-030: Config Set Command

**Epic:** EPIC-005
**Priority:** Should Have
**Points:** 3

**User Story:**
As a developer,
I want to change config values via CLI
So that I don't have to edit TOML manually.

**Acceptance Criteria:**
- [ ] `znav config set <key> <value>` updates config
- [ ] Validates key exists and value is valid type
- [ ] Persists to config file
- [ ] Shows confirmation with old and new value

**Technical Notes:**
- Use toml_edit for preserving formatting
- Validate against Config struct schema

**Dependencies:** STORY-029

---

#### STORY-031: Keyspace Migration Command

**Epic:** EPIC-005
**Priority:** Must Have
**Points:** 5

**User Story:**
As a v1.0 user,
I want to migrate from `znav:*` to `perth:*` keyspace
So that I can upgrade without losing data.

**Acceptance Criteria:**
- [ ] `znav migrate` scans znav:* keys
- [ ] Copies to perth:* with transformation
- [ ] `--dry-run` shows what would be migrated
- [ ] Reports statistics (keys migrated, errors)
- [ ] Idempotent (safe to run multiple times)

**Technical Notes:**
- Redis SCAN for key discovery
- RENAME or COPY operations
- Transaction for atomicity

**Dependencies:** None

---

#### STORY-032: Version Check on Startup

**Epic:** EPIC-005
**Priority:** Must Have
**Points:** 2

**User Story:**
As a developer,
I want Perth to check Zellij version on startup
So that I get clear errors if my Zellij is too old.

**Acceptance Criteria:**
- [ ] Parses `zellij --version` output
- [ ] Requires v0.39.0 or later
- [ ] Clear error message with upgrade instructions
- [ ] Check is fast (<100ms)

**Technical Notes:**
- Run version check in ZellijDriver::new() or lazy
- Parse semver with version crate

**Dependencies:** None

---

#### STORY-033: Goal Setting Command (Phase 4)

**Epic:** EPIC-005
**Priority:** Could Have
**Points:** 3

**User Story:**
As a developer,
I want to declare goals for panes
So that I can track progress toward completion.

**Acceptance Criteria:**
- [ ] `znav pane goal <name> "<goal>"` sets goal
- [ ] Goal stored in pane metadata
- [ ] Goal shown in tree view and history
- [ ] Goal editable and removable

**Technical Notes:**
- Store as meta:goal in pane hash
- Display in OutputFormatter

**Dependencies:** STORY-006

---

#### STORY-034: Progress Estimation (Phase 4)

**Epic:** EPIC-005
**Priority:** Could Have
**Points:** 5

**User Story:**
As a developer,
I want progress estimation based on milestones
So that I know how close I am to completing a goal.

**Acceptance Criteria:**
- [ ] `znav pane progress <name>` shows estimate
- [ ] Based on milestone count and frequency
- [ ] Shows percentage and confidence
- [ ] Works only if goal is set

**Technical Notes:**
- Simple heuristic: milestones / expected milestones
- Consider LLM for smarter estimation (future)

**Dependencies:** STORY-033

---

#### STORY-035: Search Command (Phase 4)

**Epic:** EPIC-005
**Priority:** Could Have
**Points:** 8

**User Story:**
As a developer,
I want to search across all pane histories
So that I can find related work from weeks ago.

**Acceptance Criteria:**
- [ ] `znav search "<query>"` searches all histories
- [ ] Basic text search (not semantic yet)
- [ ] Returns pane name, matching entry, relevance
- [ ] Configurable result limit (default 10)

**Technical Notes:**
- Iterate all pane histories
- Simple substring matching initially
- Semantic search requires vector DB (future story)

**Dependencies:** STORY-006

---

### Infrastructure Stories

---

#### STORY-INF-001: CI/CD Pipeline Updates

**Epic:** Infrastructure
**Priority:** Must Have
**Points:** 3

**User Story:**
As a developer,
I want CI updated for v2.0 features
So that new code is tested and validated.

**Acceptance Criteria:**
- [ ] GitHub Actions runs tests for new modules
- [ ] Redis service available in CI
- [ ] Clippy and rustfmt checks pass
- [ ] Test coverage reported

**Technical Notes:**
- Update .github/workflows/ci.yml
- Add redis service container

**Dependencies:** None

---

#### STORY-INF-002: README Updates

**Epic:** Infrastructure
**Priority:** Must Have
**Points:** 2

**User Story:**
As a potential user,
I want updated README with v2.0 features
So that I understand Perth's capabilities.

**Acceptance Criteria:**
- [ ] README reflects v2.0 intent tracking features
- [ ] Installation instructions updated
- [ ] Quick start shows log and history commands
- [ ] Links to full documentation

**Technical Notes:**
- Update existing README.md
- Add screenshots/examples

**Dependencies:** Stories complete enough to document

---

#### STORY-INF-003: Config File Template

**Epic:** Infrastructure
**Priority:** Should Have
**Points:** 2

**User Story:**
As a new user,
I want a config template with comments
So that I understand all available options.

**Acceptance Criteria:**
- [ ] `config.example.toml` in repo root
- [ ] All options documented with comments
- [ ] Sensible defaults shown
- [ ] Sensitive fields show placeholder format

**Technical Notes:**
- Create during Sprint 2 with full config schema

**Dependencies:** STORY-020

---

---

## Sprint Allocation

### Sprint 1 (Phase 1a) - 28/30 points

**Goal:** Establish intent logging foundation with data model and basic CLI commands

**Stories:**
| ID | Title | Points | Priority |
|----|-------|--------|----------|
| STORY-001 | IntentEntry Data Model | 3 | Must Have |
| STORY-002 | Redis Intent History Schema | 5 | Must Have |
| STORY-003 | CLI Log Command | 5 | Must Have |
| STORY-004 | Entry Type Classification Flag | 2 | Must Have |
| STORY-006 | CLI History Command | 5 | Must Have |
| STORY-007 | History Limit Flag | 2 | Must Have |
| STORY-031 | Keyspace Migration Command | 5 | Must Have |
| STORY-INF-001 | CI/CD Pipeline Updates | 3 | Must Have |

**Total:** 28 points / 30 capacity (93% utilization)

**Deliverables:**
- `znav pane log <name> "<summary>"` works
- `znav pane history <name>` works
- `znav migrate` handles v1.0 → v2.0 upgrade
- CI validates new code

**Risks:**
- Redis schema changes could affect v1.0 compatibility (mitigated by migration command)

**Dependencies:**
- Existing v1.0 codebase functional
- Redis available for development

---

### Sprint 2 (Phase 1b) - 27/30 points

**Goal:** Complete manual intent logging with full output formatting and documentation

**Stories:**
| ID | Title | Points | Priority |
|----|-------|--------|----------|
| STORY-005 | Artifact Tracking Flag | 3 | Should Have |
| STORY-008 | JSON Output Format | 3 | Must Have |
| STORY-009 | Human-Readable Output Formatting | 5 | Must Have |
| STORY-010 | Help Documentation for Intent Commands | 2 | Must Have |
| STORY-032 | Version Check on Startup | 2 | Must Have |
| STORY-029 | Config Show Command | 2 | Should Have |
| STORY-030 | Config Set Command | 3 | Should Have |
| STORY-INF-002 | README Updates | 2 | Must Have |
| STORY-INF-003 | Config File Template | 2 | Should Have |
| STORY-028 | Markdown Export | 3 | Should Have |

**Total:** 27 points / 30 capacity (90% utilization)

**Deliverables:**
- Full manual intent logging feature complete
- JSON and Markdown export
- Comprehensive help and documentation
- Config management via CLI

**Risks:**
- Output formatting scope creep (mitigated by clear acceptance criteria)

**Dependencies:**
- Sprint 1 complete

---

### Sprint 3 (Phase 2a) - 29/30 points

**Goal:** Establish LLM infrastructure with provider abstraction and security

**Stories:**
| ID | Title | Points | Priority |
|----|-------|--------|----------|
| STORY-011 | LLM Provider Trait | 5 | Must Have |
| STORY-012 | Anthropic Claude Provider | 5 | Must Have |
| STORY-015 | Secret Filter Module | 5 | Must Have |
| STORY-016 | Context Collector | 5 | Must Have |
| STORY-017 | CLI Snapshot Command | 5 | Must Have |
| STORY-020 | Config for LLM Settings | 3 | Must Have |
| STORY-INF-001 | (buffer) | 0 | - |

**Total:** 28 points / 30 capacity (93% utilization)

**Deliverables:**
- `znav pane snapshot <name>` works with Claude
- Secrets automatically filtered
- Context collection from shell and git

**Risks:**
- LLM API changes (mitigated by provider abstraction)
- Secret filter bypass (mitigated by fail-closed design, extensive testing)

**Dependencies:**
- Sprint 2 complete
- Anthropic API key available

---

### Sprint 4 (Phase 2b) - 22/30 points

**Goal:** Complete automated capture with alternative providers and user consent

**Stories:**
| ID | Title | Points | Priority |
|----|-------|--------|----------|
| STORY-013 | OpenAI GPT Provider | 3 | Should Have |
| STORY-014 | Local Ollama Provider | 3 | Should Have |
| STORY-018 | User Consent Flow | 3 | Must Have |
| STORY-019 | Circuit Breaker for LLM | 3 | Must Have |
| STORY-021 | Shell Hook Documentation | 2 | Should Have |
| STORY-022 | Agent Source Flag | 2 | Must Have |
| STORY-023 | Context Output Format | 3 | Must Have |
| STORY-024 | Session Resumption Display | 3 | Should Have |

**Total:** 22 points / 30 capacity (73% utilization - buffer for testing)

**Deliverables:**
- Multiple LLM providers available
- User consent workflow complete
- Graceful failure handling
- Agent integration basics

**Risks:**
- Provider API differences (mitigated by trait abstraction)

**Dependencies:**
- Sprint 3 complete

---

### Sprint 5 (Phase 3) - 25/30 points

**Goal:** Complete agent integration with Bloodbank events

**Stories:**
| ID | Title | Points | Priority |
|----|-------|--------|----------|
| STORY-025 | Bloodbank Event Publisher | 5 | Should Have |
| STORY-026 | Bloodbank Config | 2 | Should Have |
| STORY-027 | History Type Filter | 2 | Should Have |
| STORY-033 | Goal Setting Command | 3 | Could Have |
| STORY-034 | Progress Estimation | 5 | Could Have |
| STORY-035 | Search Command | 8 | Could Have |

**Total:** 25 points / 30 capacity (83% utilization)

**Deliverables:**
- Bloodbank milestone events publishing
- Basic goal tracking
- Cross-pane search

**Risks:**
- Bloodbank availability (mitigated by optional dependency)
- Search performance (mitigated by basic text search first)

**Dependencies:**
- Sprint 4 complete
- Bloodbank/RabbitMQ available for testing

---

### Sprint 6+ (Phase 4) - Future

**Goal:** Advanced intelligence features (partial, based on capacity)

**Remaining Stories (Could Have):**
- Semantic search with vector DB
- Pattern recognition
- Holocene dashboard integration
- Advanced goal tracking

**Notes:**
- Phase 4 stories are Could Have priority
- Implementation depends on Phase 1-3 success and user feedback
- Vector DB integration requires additional architectural work

---

## Epic Traceability

| Epic ID | Epic Name | Stories | Total Points | Sprint(s) | Status |
|---------|-----------|---------|--------------|-----------|--------|
| EPIC-001 | v1.0 Navigation Primitives | (implemented) | - | - | ✅ Complete |
| EPIC-002 | Manual Intent Logging | STORY-001 to 010, 028-032 | 45 | Sprint 1-2 | Phase 1 |
| EPIC-003 | Automated Context Capture | STORY-011 to 021 | 47 | Sprint 3-4 | Phase 2 |
| EPIC-004 | Agent Integration | STORY-022 to 027 | 17 | Sprint 4-5 | Phase 3 |
| EPIC-005 | Advanced Intelligence | STORY-033 to 035 | 16 | Sprint 5-6 | Phase 4 |
| Infrastructure | CI, Docs, Config | STORY-INF-001 to 003 | 7 | Sprint 1-2 | Support |

---

## Requirements Coverage

### Functional Requirements

| FR ID | FR Name | Story | Sprint |
|-------|---------|-------|--------|
| FR-001 | Manual Intent Logging | STORY-003 | 1 |
| FR-002 | Intent History Retrieval | STORY-006 | 1 |
| FR-003 | Intent Entry Classification | STORY-004 | 1 |
| FR-004 | Artifact Tracking | STORY-005 | 2 |
| FR-005 | Multiple Output Formats | STORY-008, 009, 028 | 2 |
| FR-006 | LLM Auto-Summarization | STORY-017 | 3 |
| FR-007 | Shell Hook Integration | STORY-021 | 4 |
| FR-008 | Secret Pattern Filtering | STORY-015 | 3 |
| FR-009 | Local Model Fallback | STORY-014 | 4 |
| FR-010 | Configurable Snapshot Triggers | STORY-020 | 3 |
| FR-011 | Context Recovery API | STORY-023 | 4 |
| FR-012 | Bloodbank Event Publishing | STORY-025 | 5 |
| FR-013 | Agent Checkpoint Recording | STORY-022 | 4 |
| FR-014 | Session Resumption Context | STORY-024 | 4 |
| FR-015 | Semantic Search | (Phase 4+) | Future |
| FR-016 | Goal State Tracking | STORY-033 | 5 |
| FR-017 | Pattern Recognition | (Phase 4+) | Future |
| FR-018 | Dashboard Visualization | (Phase 4+) | Future |
| FR-019 | Export to Markdown | STORY-028 | 2 |
| FR-020 | Pane-First Navigation | (v1.0) | Complete |
| FR-021 | Redis-Backed State | (v1.0) | Complete |
| FR-022 | Auto-Focus via Position | (v1.0) | Complete |
| FR-023 | Metadata Attachment | (v1.0) | Complete |
| FR-024 | Reconciliation | (v1.0) | Complete |
| FR-025 | Tree Visualization | (v1.0) | Complete |

**Coverage:** 22/25 FRs covered in Sprint 1-5 (88%)
**Deferred:** FR-015, FR-017, FR-018 (Could Have, Phase 4+)

---

## Risks and Mitigation

### High Risk

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Secret filter bypass | Credential leakage | Low | Fail-closed design, fuzz testing, security audit |
| LLM cost overrun | Budget impact | Medium | Rate limiting, local fallback, cost monitoring |

### Medium Risk

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| v1.0 migration issues | Data loss | Low | Migration command, dry-run mode, backup docs |
| LLM provider API changes | Feature breakage | Medium | Provider abstraction, version pinning |
| Bloodbank unavailability | Event loss | Medium | Optional dependency, graceful degradation |

### Low Risk

| Risk | Impact | Probability | Mitigation |
|------|--------|-------------|------------|
| Zellij API changes | Compatibility issues | Low | Version check, CI against multiple versions |
| Shell hook performance | UX degradation | Low | Async execution, rate limiting, benchmarks |

---

## Dependencies

### External Dependencies

| Dependency | Required For | Status |
|------------|--------------|--------|
| Redis 6.0+ | All sprints | Available |
| Zellij v0.39+ | All sprints | Available |
| Anthropic API | Sprint 3+ | API key required |
| OpenAI API | Sprint 4 | API key required (optional) |
| Ollama | Sprint 4 | Local install (optional) |
| RabbitMQ (Bloodbank) | Sprint 5 | Available in 33GOD infra |

### Internal Dependencies

| Dependency | Required For | Notes |
|------------|--------------|-------|
| STORY-001 | STORY-002, 003 | Data model first |
| STORY-002 | STORY-003, 006 | Schema before commands |
| STORY-011 | STORY-012, 013, 014, 017 | Trait before providers |
| STORY-015 | STORY-017 | Filter before snapshot |

---

## Definition of Done

For a story to be considered complete:

- [ ] Code implemented and committed to feature branch
- [ ] Unit tests written and passing (≥80% coverage for new code)
- [ ] Integration tests passing (where applicable)
- [ ] Code reviewed via PR (self-review acceptable for solo)
- [ ] No clippy warnings, rustfmt clean
- [ ] Documentation updated (help text, README if user-facing)
- [ ] Performance validated against targets (latency benchmarks)
- [ ] Merged to main branch

---

## Sprint Cadence

**Sprint Length:** 2 weeks
**Sprint Planning:** Day 1 (Monday)
**Daily Standups:** N/A (solo project, use todo list)
**Sprint Review:** Day 10 (Friday)
**Sprint Retrospective:** Day 10 (Friday, brief notes)

**Velocity Tracking:**
- Sprint 1: Target 28 points
- Adjust subsequent sprints based on actual velocity

---

## Next Steps

**Immediate:** Begin Sprint 1

Run `/dev-story STORY-001` to start implementing the IntentEntry data model.

**Sprint 1 Story Order:**
1. STORY-001: IntentEntry Data Model (foundation)
2. STORY-002: Redis Intent History Schema
3. STORY-003: CLI Log Command
4. STORY-004: Entry Type Classification Flag
5. STORY-006: CLI History Command
6. STORY-007: History Limit Flag
7. STORY-031: Keyspace Migration Command
8. STORY-INF-001: CI/CD Pipeline Updates

**Commands:**
- `/dev-story STORY-XXX` - Implement specific story
- `/create-story STORY-XXX` - Generate detailed story document
- `/sprint-status` - Check current sprint progress

---

**This plan was created using BMAD Method v6 - Phase 4 (Implementation Planning)**

*To continue: Run `/workflow-status` to see your progress and next recommended workflow.*
