use crate::bloodbank::EventPublisher;
use crate::context::ContextCollector;
use crate::llm::{create_provider, CircuitBreaker, LLMConfig};
use crate::state::{MigrationResult, StateManager};
use crate::types::{IntentEntry, IntentSource, IntentType, PaneInfoOutput, PaneRecord, PaneStatus, TabRecord};
use crate::zellij::ZellijDriver;
use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;
use std::time::Duration;
use tokio::time::timeout;

/// Global circuit breaker for LLM API calls.
/// Prevents cascading failures by tracking consecutive errors.
static LLM_CIRCUIT_BREAKER: LazyLock<CircuitBreaker> = LazyLock::new(CircuitBreaker::new);

const CURRENT_TAB: &str = "current";

pub struct Orchestrator {
    state: StateManager,
    zellij: ZellijDriver,
    events: EventPublisher,
}

impl Orchestrator {
    pub fn new(state: StateManager, zellij: ZellijDriver, events: EventPublisher) -> Self {
        Self { state, zellij, events }
    }

    pub async fn open_pane(
        &mut self,
        pane_name: String,
        tab: Option<String>,
        session: Option<String>,
        meta: HashMap<String, String>,
        show_last_intent: bool,
    ) -> Result<()> {
        if let Some(record) = self.state.get_pane(&pane_name).await? {
            return self.open_existing_pane(record, session, meta, show_last_intent).await;
        }

        self.create_pane(pane_name, tab, session, meta).await
    }

    pub async fn pane_info(&mut self, pane_name: String) -> Result<PaneInfoOutput> {
        match self.state.get_pane(&pane_name).await? {
            Some(record) => {
                let status = if record.stale {
                    PaneStatus::Stale
                } else {
                    PaneStatus::Found
                };
                Ok(PaneInfoOutput {
                    pane_name: record.pane_name,
                    session: record.session,
                    tab: record.tab,
                    pane_id: record.pane_id,
                    created_at: record.created_at,
                    last_seen: record.last_seen,
                    last_accessed: record.last_accessed,
                    meta: record.meta,
                    status,
                    source: "redis".to_string(),
                })
            }
            None => Ok(PaneInfoOutput::missing(pane_name)),
        }
    }

    pub async fn ensure_tab(&self, tab_name: &str) -> Result<bool> {
        let tabs = self.zellij.query_tab_names(None).await?;
        if tabs.iter().any(|tab| tab == tab_name) {
            self.zellij.go_to_tab_name(None, tab_name).await?;
            Ok(false)
        } else {
            self.zellij
                .new_tab(None, tab_name)
                .await
                .context("failed to create tab")?;
            Ok(true)
        }
    }

    /// Create a new tab with optional correlation ID for event traceability.
    ///
    /// The correlation ID is appended to the tab name as a suffix (e.g., "myapp(fixes)-pr-42")
    /// and stored in Redis for later querying.
    ///
    /// Returns a TabCreateResult indicating whether the tab was created or already exists.
    pub async fn create_tab(
        &mut self,
        tab_name: String,
        correlation_id: Option<String>,
        meta: HashMap<String, String>,
    ) -> Result<TabCreateResult> {
        // Determine the target session
        let target_session = self
            .zellij
            .active_session_name()
            .ok_or_else(|| anyhow!("no active session; must be inside a Zellij session"))?;

        // Compute the effective tab name (with correlation ID suffix if provided)
        let effective_name = match &correlation_id {
            Some(id) => format!("{}-{}", tab_name, id),
            None => tab_name.clone(),
        };

        // Check if tab already exists in Zellij
        let tabs = self.zellij.query_tab_names(None).await?;
        if tabs.iter().any(|tab| tab == &effective_name) {
            // Tab already exists - touch it and focus
            self.zellij.go_to_tab_name(None, &effective_name).await?;
            self.state.touch_tab(&effective_name, &target_session).await?;

            return Ok(TabCreateResult {
                tab_name: effective_name,
                correlation_id,
                created: false,
                session: target_session,
            });
        }

        // Create the tab in Zellij
        self.zellij
            .new_tab(None, &effective_name)
            .await
            .context("failed to create tab in Zellij")?;

        // Store in Redis
        let now = StateManager::now_string();
        let mut record = TabRecord::new(effective_name.clone(), target_session.clone(), now);

        if let Some(id) = &correlation_id {
            record = record.with_correlation_id(id);
        }

        if !meta.is_empty() {
            record = record.with_meta(meta);
        }

        self.state.upsert_tab(&record).await?;

        // Publish tab.created event
        self.events.tab_created(&record).await;

        Ok(TabCreateResult {
            tab_name: effective_name,
            correlation_id,
            created: true,
            session: target_session,
        })
    }

    /// Get info about a tab by name.
    pub async fn tab_info(&mut self, tab_name: &str) -> Result<Option<TabRecord>> {
        let session = self
            .zellij
            .active_session_name()
            .ok_or_else(|| anyhow!("no active session; must be inside a Zellij session"))?;

        self.state.get_tab(tab_name, &session).await
    }

    /// Spawn multiple named panes in a single tab (STORY-037).
    ///
    /// Creates multiple panes sequentially in the specified tab, naming each one
    /// according to the provided list. Each pane is registered in Redis with
    /// position metadata for later focus restoration.
    ///
    /// # Arguments
    /// * `tab_name` - The tab to create panes in (will be created if it doesn't exist)
    /// * `pane_names` - Names for each pane to create
    /// * `cwds` - Optional working directories for each pane (shorter list is padded with None)
    /// * `vertical` - If true, creates vertical splits (side by side); if false, horizontal (stacked)
    ///
    /// # Returns
    /// A `BatchResult` containing the list of created and skipped panes.
    pub async fn batch_panes(
        &mut self,
        tab_name: String,
        pane_names: Vec<String>,
        cwds: Vec<String>,
        vertical: bool,
    ) -> Result<BatchResult> {
        if pane_names.is_empty() {
            return Err(anyhow!("at least one pane name is required"));
        }

        // Determine the target session
        let target_session = self
            .zellij
            .active_session_name()
            .ok_or_else(|| anyhow!("no active session; must be inside a Zellij session"))?;

        // Ensure tab exists (creates it if needed)
        let tab_created = self.ensure_tab_in_session(None, &tab_name).await?;

        let mut panes_created = Vec::new();
        let mut panes_skipped = Vec::new();

        let direction = if vertical { "right" } else { "down" };

        for (idx, pane_name) in pane_names.iter().enumerate() {
            // Check if pane already exists in Redis
            if self.state.get_pane(pane_name).await?.is_some() {
                panes_skipped.push(pane_name.clone());
                continue;
            }

            // Get cwd for this pane (if provided)
            let cwd = cwds.get(idx).cloned();

            if idx == 0 && tab_created {
                // First pane in a newly created tab - just rename the initial pane
                self.zellij.rename_pane(None, pane_name).await?;
            } else {
                // Create a new pane with split direction
                if let Some(ref cwd_path) = cwd {
                    // Resolve to absolute path
                    let abs_cwd = std::fs::canonicalize(cwd_path)
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| cwd_path.clone());
                    self.zellij
                        .new_pane_with_cwd(None, &abs_cwd, direction)
                        .await?;
                } else if vertical {
                    self.zellij.new_pane_vertical(None).await?;
                } else {
                    self.zellij.new_pane_horizontal(None).await?;
                }
                self.zellij.rename_pane(None, pane_name).await?;
            }

            // Store pane in Redis with position metadata
            let now = StateManager::now_string();
            let mut meta = HashMap::new();
            meta.insert("position".to_string(), idx.to_string());
            if let Some(ref cwd_path) = cwd {
                // Store resolved path in metadata
                let abs_cwd = std::fs::canonicalize(cwd_path)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| cwd_path.clone());
                meta.insert("cwd".to_string(), abs_cwd);
            }

            let record = PaneRecord::new(
                pane_name.clone(),
                target_session.clone(),
                tab_name.clone(),
                now,
                meta,
            );
            self.state.upsert_pane(&record).await?;

            // Publish pane.created event
            self.events.pane_created(&record).await;

            panes_created.push(pane_name.clone());
        }

        Ok(BatchResult {
            tab_name,
            panes_created,
            panes_skipped,
            session: target_session,
        })
    }

    pub async fn reconcile(&mut self) -> Result<()> {
        let current_session = self
            .zellij
            .active_session_name()
            .ok_or_else(|| anyhow!("not inside a zellij session; reconcile requires one"))?;

        let mut layout_panes = HashSet::new();
        let mut layout_confident = false;
        if let Some(layout) = self.zellij.dump_layout_json(None).await? {
            collect_pane_names(&layout, &mut layout_panes, false);
            if !layout_panes.is_empty() {
                layout_confident = true;
            }
        }

        let pane_names = self.state.list_pane_names().await?;
        let mut total = 0;
        let mut seen = 0;
        let mut stale = 0;
        let mut skipped = 0;

        for pane_name in pane_names {
            total += 1;
            let Some(record) = self.state.get_pane(&pane_name).await? else {
                skipped += 1;
                continue;
            };

            if record.session != current_session {
                skipped += 1;
                continue;
            }

            if !layout_confident {
                skipped += 1;
                continue;
            }

            if layout_panes.contains(&record.pane_name) {
                self.state.mark_seen(&record.pane_name).await?;
                seen += 1;
            } else {
                self.state.mark_stale(&record.pane_name).await?;
                stale += 1;
            }
        }

        println!(
            "reconcile: session={} total={} seen={} stale={} skipped={}",
            current_session, total, seen, stale, skipped
        );

        Ok(())
    }

    async fn open_existing_pane(
        &mut self,
        record: PaneRecord,
        session: Option<String>,
        meta: HashMap<String, String>,
        show_last_intent: bool,
    ) -> Result<()> {
        if let Some(requested_session) = session {
            if requested_session != record.session {
                return Err(anyhow!(
                    "pane '{}' already belongs to session '{}'",
                    record.pane_name,
                    record.session
                ));
            }
        }

        let action_session = self.ensure_session(&record.session).await?;

        if !record.tab.is_empty() && record.tab != CURRENT_TAB {
            if let Err(err) = self
                .zellij
                .go_to_tab_name(action_session.as_deref(), &record.tab)
                .await
            {
                self.state.mark_stale(&record.pane_name).await?;
                return Err(err).context("failed to switch to pane tab; marked stale")?;
            }

            // Auto-focus pane by position if stored
            if let Some(position_str) = record.meta.get("position") {
                if let Ok(position) = position_str.parse::<usize>() {
                    if let Err(err) = self
                        .zellij
                        .focus_pane_by_index(action_session.as_deref(), position)
                        .await
                    {
                        // Log warning but don't fail - tab is focused, pane focus is best-effort
                        eprintln!(
                            "Warning: Could not focus pane '{}' at position {}: {}",
                            record.pane_name, position, err
                        );
                    }
                }
            }
        }

        self.state.touch_pane(&record.pane_name, &meta).await?;

        // Publish pane.opened event
        self.events.pane_opened(&record.pane_name, &record.tab, &record.session).await;

        // Show last intent on resume if enabled and history exists
        if show_last_intent {
            if let Ok(history) = self.state.get_history(&record.pane_name, Some(1)).await {
                if let Some(last_entry) = history.first() {
                    self.display_resume_context(&record.pane_name, last_entry);
                }
            }
        }

        Ok(())
    }

    /// Display a brief resume context when returning to a pane.
    fn display_resume_context(&self, _pane_name: &str, entry: &IntentEntry) {
        use chrono::{Local, TimeZone};
        use chrono_humanize::HumanTime;

        // Convert to local time for relative display
        let local_time = Local.from_utc_datetime(&entry.timestamp.naive_utc());
        let human_time = HumanTime::from(local_time);

        // Determine type icon
        let type_icon = match entry.entry_type {
            IntentType::Milestone => "★",
            IntentType::Checkpoint => "●",
            IntentType::Exploration => "◈",
        };

        // Source indicator
        let source_indicator = match entry.source {
            IntentSource::Agent => " 🤖",
            IntentSource::Automated => " ⚡",
            IntentSource::Manual => "",
        };

        // Check if terminal supports color
        use std::io::IsTerminal;
        let use_color = std::env::var("NO_COLOR").is_err() && std::io::stderr().is_terminal();

        if use_color {
            use colored::Colorize;
            eprintln!(
                "{} {} {} {}{}",
                "Resuming:".cyan(),
                type_icon.yellow(),
                entry.summary.white(),
                human_time.to_string().dimmed(),
                source_indicator
            );
        } else {
            eprintln!(
                "Resuming: {} {} ({}){}",
                type_icon, entry.summary, human_time, source_indicator
            );
        }
    }

    async fn create_pane(
        &mut self,
        pane_name: String,
        tab: Option<String>,
        session: Option<String>,
        meta: HashMap<String, String>,
    ) -> Result<()> {
        let target_session = match session {
            Some(session) => session,
            None => self
                .zellij
                .active_session_name()
                .ok_or_else(|| anyhow!("no active session; pass --session"))?,
        };

        let action_session = self.ensure_session(&target_session).await?;

        let mut created_tab = false;
        let final_tab = if let Some(tab_name) = tab {
            created_tab = self.ensure_tab_in_session(action_session.as_deref(), &tab_name).await?;
            tab_name
        } else {
            CURRENT_TAB.to_string()
        };

        // Capture pane position before creating the new pane
        let position = if final_tab != CURRENT_TAB {
            self.count_panes_in_tab(action_session.as_deref(), &final_tab)
                .await
                .unwrap_or(0)
        } else {
            0 // For current tab, position tracking is unreliable; use 0 as fallback
        };

        if created_tab {
            self.zellij
                .rename_pane(action_session.as_deref(), &pane_name)
                .await?;
        } else {
            self.zellij.new_pane(action_session.as_deref()).await?;
            self.zellij
                .rename_pane(action_session.as_deref(), &pane_name)
                .await?;
        }

        // Store position in metadata
        let mut meta_with_position = meta;
        meta_with_position.insert("position".to_string(), position.to_string());

        let now = StateManager::now_string();
        let record = PaneRecord::new(pane_name, target_session, final_tab, now, meta_with_position);
        self.state.upsert_pane(&record).await?;

        // Publish pane.created event
        self.events.pane_created(&record).await;

        Ok(())
    }

    async fn ensure_session(&self, target_session: &str) -> Result<Option<String>> {
        if let Some(current) = self.zellij.active_session_name() {
            if current == target_session {
                return Ok(None);
            }
            return Err(anyhow!(
                "target session '{}' is not active (current '{}'); detach and retry",
                target_session,
                current
            ));
        }

        match self.zellij.query_tab_names(Some(target_session)).await {
            Ok(_) => Ok(Some(target_session.to_string())),
            Err(_) => {
                self.zellij.attach_session(target_session).await?;
                Err(anyhow!(
                    "attached to session '{}'; re-run command to continue",
                    target_session
                ))
            }
        }
    }

    async fn count_panes_in_tab(
        &self,
        session: Option<&str>,
        tab_name: &str,
    ) -> Result<usize> {
        let layout = self.zellij.dump_layout_json(session).await?;

        if let Some(layout_value) = layout {
            let count = count_panes_in_tab_from_layout(&layout_value, tab_name);
            Ok(count)
        } else {
            // Fallback: if layout not available, assume 0 (will be 1 after creation)
            Ok(0)
        }
    }

    async fn ensure_tab_in_session(
        &self,
        session: Option<&str>,
        tab_name: &str,
    ) -> Result<bool> {
        let tabs = self.zellij.query_tab_names(session).await?;
        if tabs.iter().any(|tab| tab == tab_name) {
            self.zellij.go_to_tab_name(session, tab_name).await?;
            Ok(false)
        } else {
            self.zellij
                .new_tab(session, tab_name)
                .await
                .context("failed to create tab")?;
            Ok(true)
        }
    }

    // ========================================================================
    // Intent History Methods (Perth v2.0)
    // ========================================================================

    /// Log an intent entry for a pane
    pub async fn log_intent(&mut self, pane_name: &str, entry: &IntentEntry) -> Result<()> {
        self.state.log_intent(pane_name, entry).await?;

        // Publish intent.logged event (and milestone.recorded if applicable)
        let session = self.zellij.active_session_name();
        self.events.intent_logged(pane_name, entry, session.as_deref()).await;

        Ok(())
    }

    /// Get intent history for a pane
    pub async fn get_history(&mut self, pane_name: &str, limit: Option<usize>) -> Result<Vec<IntentEntry>> {
        self.state.get_history(pane_name, limit).await
    }

    /// Generate an LLM-powered snapshot of recent work
    ///
    /// Requires user consent to be granted before sending data to an LLM provider.
    /// The 'none' provider does not require consent (no data is sent).
    ///
    /// Uses a circuit breaker to prevent cascading failures:
    /// - Opens after 3 consecutive failures
    /// - Half-opens after 5 minute cooldown
    /// - Single success closes the circuit
    pub async fn snapshot(&mut self, pane_name: &str, llm_config: &LLMConfig, consent_given: bool) -> Result<SnapshotResult> {
        const SNAPSHOT_TIMEOUT: Duration = Duration::from_secs(30);

        // Check circuit breaker first (before any expensive operations)
        if llm_config.provider != "none" {
            LLM_CIRCUIT_BREAKER.allow_request().map_err(|msg| anyhow!("{}", msg))?;
        }

        // Create LLM provider
        let provider = create_provider(llm_config);
        if !provider.is_available() {
            return Err(anyhow!(
                "LLM provider '{}' is not available. Configure API key or use a different provider.",
                llm_config.provider
            ));
        }

        // Check consent for providers that send data externally
        // The 'none' provider doesn't send data, so it doesn't require consent
        if llm_config.provider != "none" && !consent_given {
            return Err(anyhow!(
                "LLM consent not granted.\n\n\
                The snapshot command sends shell history, git diff, and file information\n\
                to '{}' for AI-powered summarization.\n\n\
                To grant consent, run:\n\
                  zdrive config consent --grant\n\n\
                To see what data would be sent:\n\
                  zdrive config consent --help",
                llm_config.provider
            ));
        }

        // Collect context
        let collector = ContextCollector::new()
            .context("failed to create context collector")?;

        let cwd = std::env::current_dir().ok();
        let context = collector
            .collect(pane_name, cwd.as_deref())
            .context("failed to collect context")?;

        // Get existing summary if any (to provide continuity)
        let existing = self.state.get_history(pane_name, Some(1)).await.ok()
            .and_then(|h| h.into_iter().next())
            .map(|e| e.summary);

        let context = if let Some(summary) = existing {
            context.with_existing_summary(summary)
        } else {
            context
        };

        // Call LLM with timeout and track circuit breaker state
        let llm_result = timeout(SNAPSHOT_TIMEOUT, provider.summarize(&context)).await;

        // Handle the result and update circuit breaker
        let result = match llm_result {
            Ok(Ok(result)) => {
                // Success - close the circuit
                if llm_config.provider != "none" {
                    LLM_CIRCUIT_BREAKER.record_success();
                }
                result
            }
            Ok(Err(e)) => {
                // LLM error - record failure
                if llm_config.provider != "none" {
                    LLM_CIRCUIT_BREAKER.record_failure();
                }
                return Err(e).context("LLM summarization failed");
            }
            Err(_) => {
                // Timeout - record failure
                if llm_config.provider != "none" {
                    LLM_CIRCUIT_BREAKER.record_failure();
                }
                return Err(anyhow!(
                    "LLM request timed out after {} seconds.\n\n\
                    You can still log entries manually:\n\
                    zdrive pane log {} \"<your summary>\"",
                    SNAPSHOT_TIMEOUT.as_secs(),
                    pane_name
                ));
            }
        };

        // Determine entry type from LLM suggestion
        let entry_type = match result.suggested_type.as_deref() {
            Some("milestone") => IntentType::Milestone,
            Some("exploration") => IntentType::Exploration,
            _ => IntentType::Checkpoint,
        };

        // Create and store the intent entry
        let entry = IntentEntry::new(&result.summary)
            .with_type(entry_type)
            .with_source(IntentSource::Automated)
            .with_artifacts(result.key_files.clone());

        self.state.log_intent(pane_name, &entry).await
            .context("failed to log generated intent")?;

        Ok(SnapshotResult {
            summary: result.summary,
            entry_type,
            key_files: result.key_files,
            tokens_used: result.tokens_used,
        })
    }

    /// Migrate from v1.0 (znav:*) to v2.0 (perth:*) keyspace
    pub async fn migrate_keyspace(&mut self, dry_run: bool) -> Result<MigrationResult> {
        self.state.migrate_keyspace(dry_run).await
    }

    pub async fn visualize(&mut self) -> Result<()> {
        let panes = self.state.list_all_panes().await?;

        if panes.is_empty() {
            println!("No panes tracked in Redis");
            return Ok(());
        }

        // Organize panes by session -> tab
        let mut sessions: HashMap<String, HashMap<String, Vec<PaneRecord>>> = HashMap::new();
        for pane in panes {
            sessions
                .entry(pane.session.clone())
                .or_default()
                .entry(pane.tab.clone())
                .or_default()
                .push(pane);
        }

        // Sort sessions for consistent output
        let mut session_names: Vec<_> = sessions.keys().cloned().collect();
        session_names.sort();

        for (session_idx, session_name) in session_names.iter().enumerate() {
            let is_last_session = session_idx == session_names.len() - 1;
            let tabs = sessions.get(session_name).unwrap();

            // Print session header
            println!("{}", session_name);

            // Sort tabs for consistent output
            let mut tab_names: Vec<_> = tabs.keys().cloned().collect();
            tab_names.sort();

            for (tab_idx, tab_name) in tab_names.iter().enumerate() {
                let is_last_tab = tab_idx == tab_names.len() - 1;
                let panes_in_tab = tabs.get(tab_name).unwrap();

                // Look up tab in Redis to get correlation ID
                let correlation_id = self.state.get_tab(tab_name, session_name).await
                    .ok()
                    .flatten()
                    .and_then(|tab| tab.correlation_id);

                // Print tab with correlation ID if present
                let tab_prefix = if is_last_session && is_last_tab {
                    "└──"
                } else {
                    "├──"
                };

                let tab_display = match correlation_id {
                    Some(ref id) => format!("{} [{}]", tab_name, id),
                    None => tab_name.to_string(),
                };
                println!("{} {}", tab_prefix, tab_display);

                // Sort panes by name for consistent output
                let mut sorted_panes = panes_in_tab.clone();
                sorted_panes.sort_by(|a, b| a.pane_name.cmp(&b.pane_name));

                for (pane_idx, pane) in sorted_panes.iter().enumerate() {
                    let is_last_pane = pane_idx == sorted_panes.len() - 1;

                    // Determine the correct tree characters
                    let pane_prefix = if is_last_session && is_last_tab {
                        if is_last_pane {
                            "    └──"
                        } else {
                            "    ├──"
                        }
                    } else {
                        if is_last_pane {
                            "│   └──"
                        } else {
                            "│   ├──"
                        }
                    };

                    // Build pane display line with status indicator
                    let status_indicator = if pane.stale { "[stale]" } else { "" };
                    let pane_line = format!("{} {}", pane.pane_name, status_indicator).trim().to_string();

                    println!("{} {}", pane_prefix, pane_line);

                    // Show metadata if present
                    if !pane.meta.is_empty() {
                        let meta_prefix = if is_last_session && is_last_tab {
                            if is_last_pane {
                                "        "
                            } else {
                                "    │   "
                            }
                        } else {
                            if is_last_pane {
                                "│       "
                            } else {
                                "│   │   "
                            }
                        };

                        let mut meta_items: Vec<_> = pane.meta.iter().collect();
                        meta_items.sort_by_key(|(k, _)| *k);

                        for (key, value) in meta_items {
                            println!("{}  {}={}", meta_prefix, key, value);
                        }
                    }
                }
            }

            // Add spacing between sessions
            if !is_last_session {
                println!();
            }
        }

        Ok(())
    }
}

fn collect_pane_names(value: &Value, panes: &mut HashSet<String>, in_pane_list: bool) {
    match value {
        Value::Object(map) => {
            if in_pane_list {
                if let Some(name) = map
                    .get("pane_name")
                    .and_then(|v| v.as_str())
                    .or_else(|| map.get("name").and_then(|v| v.as_str()))
                {
                    panes.insert(name.to_string());
                }
            } else if let Some(name) = map.get("pane_name").and_then(|v| v.as_str()) {
                panes.insert(name.to_string());
            }

            for (key, child) in map {
                let child_in_pane_list = matches!(key.as_str(), "panes" | "floating_panes");
                collect_pane_names(child, panes, child_in_pane_list);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_pane_names(item, panes, in_pane_list);
            }
        }
        _ => {}
    }
}

fn count_panes_in_tab_from_layout(layout: &Value, target_tab: &str) -> usize {
    // Navigate to the target tab in the layout and count panes
    if let Some(tabs) = layout.get("tabs").and_then(|v| v.as_array()) {
        for tab in tabs {
            if let Some(tab_name) = tab.get("name").and_then(|v| v.as_str()) {
                if tab_name == target_tab {
                    return count_panes_recursive(tab);
                }
            }
        }
    }
    0
}

fn count_panes_recursive(value: &Value) -> usize {
    match value {
        Value::Object(map) => {
            // Check if this is a pane object (has pane_name or is in panes array)
            let mut count = 0;

            // Count panes in "panes" array
            if let Some(panes) = map.get("panes").and_then(|v| v.as_array()) {
                for pane in panes {
                    count += count_panes_recursive(pane);
                }
            }

            // Count floating panes
            if let Some(floating) = map.get("floating_panes").and_then(|v| v.as_array()) {
                for pane in floating {
                    count += count_panes_recursive(pane);
                }
            }

            // If this object has a pane_name, it's a pane itself
            if map.contains_key("pane_name") || map.contains_key("name") {
                count += 1;
            }

            count
        }
        Value::Array(items) => {
            items.iter().map(count_panes_recursive).sum()
        }
        _ => 0,
    }
}

/// Result of a snapshot operation
#[derive(Debug, Clone)]
pub struct SnapshotResult {
    /// The generated summary
    pub summary: String,
    /// The entry type determined by the LLM
    pub entry_type: IntentType,
    /// Key files identified
    pub key_files: Vec<String>,
    /// Tokens used (for cost tracking)
    pub tokens_used: Option<u32>,
}

/// Result of a tab create operation (STORY-036)
#[derive(Debug, Clone)]
pub struct TabCreateResult {
    /// The effective tab name (may include correlation ID suffix)
    pub tab_name: String,
    /// The correlation ID if one was provided
    pub correlation_id: Option<String>,
    /// Whether the tab was newly created (false if already existed)
    pub created: bool,
    /// The session the tab belongs to
    pub session: String,
}

/// Result of a batch pane operation (STORY-037)
#[derive(Debug, Clone)]
pub struct BatchResult {
    /// The tab name where panes were created
    pub tab_name: String,
    /// Names of panes that were created
    pub panes_created: Vec<String>,
    /// Names of panes that already existed (skipped)
    pub panes_skipped: Vec<String>,
    /// The session the panes belong to
    pub session: String,
}
