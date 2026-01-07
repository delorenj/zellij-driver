use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// Intent Tracking Types (Perth v2.0)
// ============================================================================

/// Classification of intent entries by their significance level.
///
/// - `Milestone`: Major accomplishment or significant progress point
/// - `Checkpoint`: Regular progress marker during work
/// - `Exploration`: Investigative or research-oriented activity
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum IntentType {
    /// Major accomplishment or significant progress point
    Milestone,
    /// Regular progress marker during work (default)
    Checkpoint,
    /// Investigative or research-oriented activity
    Exploration,
}

impl Default for IntentType {
    fn default() -> Self {
        Self::Checkpoint
    }
}

/// Source of the intent entry - how it was created.
///
/// - `Manual`: User explicitly logged via CLI command
/// - `Automated`: System-generated based on activity detection
/// - `Agent`: Created by an AI agent during assisted workflow
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "lowercase")]
pub enum IntentSource {
    /// User manually logged this entry (default)
    Manual,
    /// System-generated from activity detection
    Automated,
    /// Created by an AI agent
    Agent,
}

impl Default for IntentSource {
    fn default() -> Self {
        Self::Manual
    }
}

/// Core data structure for tracking developer intent and cognitive context.
///
/// Each IntentEntry captures what the developer was working on at a point in time,
/// including their goal, artifacts touched, and progress indicators.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentEntry {
    /// Unique identifier for this entry
    pub id: Uuid,
    /// When this intent was logged
    pub timestamp: DateTime<Utc>,
    /// Human-readable summary of what was being worked on
    pub summary: String,
    /// Classification of this entry's significance
    #[serde(default)]
    pub entry_type: IntentType,
    /// Files, URLs, or other artifacts referenced during this work
    #[serde(default)]
    pub artifacts: Vec<String>,
    /// Number of commands executed during this intent period (if tracked)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commands_run: Option<usize>,
    /// Description of progress made toward the goal
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal_delta: Option<String>,
    /// How this entry was created
    #[serde(default)]
    pub source: IntentSource,
}

impl IntentEntry {
    /// Create a new IntentEntry with the given summary.
    /// Generates a new UUID and sets timestamp to now.
    pub fn new(summary: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            summary: summary.into(),
            entry_type: IntentType::default(),
            artifacts: Vec::new(),
            commands_run: None,
            goal_delta: None,
            source: IntentSource::default(),
        }
    }

    /// Builder method to set the entry type
    pub fn with_type(mut self, entry_type: IntentType) -> Self {
        self.entry_type = entry_type;
        self
    }

    /// Builder method to set artifacts
    pub fn with_artifacts(mut self, artifacts: Vec<String>) -> Self {
        self.artifacts = artifacts;
        self
    }

    /// Builder method to set source
    pub fn with_source(mut self, source: IntentSource) -> Self {
        self.source = source;
        self
    }

    /// Builder method to set goal delta
    pub fn with_goal_delta(mut self, delta: impl Into<String>) -> Self {
        self.goal_delta = Some(delta.into());
        self
    }

    /// Builder method to set commands run count
    pub fn with_commands_run(mut self, count: usize) -> Self {
        self.commands_run = Some(count);
        self
    }

    /// Get a human-readable string for the entry type
    pub fn entry_type_str(&self) -> &'static str {
        match self.entry_type {
            IntentType::Milestone => "MILESTONE",
            IntentType::Checkpoint => "CHECKPOINT",
            IntentType::Exploration => "EXPLORATION",
        }
    }

    /// Get a lowercase string for the source (for event payloads)
    pub fn source_str(&self) -> &'static str {
        match self.source {
            IntentSource::Manual => "manual",
            IntentSource::Automated => "automated",
            IntentSource::Agent => "agent",
        }
    }
}

// ============================================================================
// Tab Tracking Types (Perth v2.0 - STORY-036)
// ============================================================================

/// Record for tracking tab metadata with correlation ID support.
///
/// Enables traceability for agentic workflows by associating tabs with
/// correlation IDs from triggering events (e.g., Bloodbank events).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabRecord {
    /// Tab name (may include correlation ID suffix)
    pub tab_name: String,
    /// Session this tab belongs to
    pub session: String,
    /// Optional correlation ID for event traceability
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    /// When this tab was created
    pub created_at: String,
    /// Last time this tab was accessed
    pub last_accessed: String,
    /// Additional metadata key-value pairs
    #[serde(default)]
    pub meta: HashMap<String, String>,
}

impl TabRecord {
    /// Create a new TabRecord with the given name and session.
    pub fn new(tab_name: String, session: String, now: String) -> Self {
        Self {
            tab_name,
            session,
            correlation_id: None,
            created_at: now.clone(),
            last_accessed: now,
            meta: HashMap::new(),
        }
    }

    /// Builder method to set correlation ID
    pub fn with_correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.correlation_id = Some(correlation_id.into());
        self
    }

    /// Builder method to set metadata
    pub fn with_meta(mut self, meta: HashMap<String, String>) -> Self {
        self.meta = meta;
        self
    }

    /// Get the effective tab name (with correlation ID suffix if present)
    pub fn effective_name(&self) -> String {
        match &self.correlation_id {
            Some(id) => format!("{}-{}", self.tab_name, id),
            None => self.tab_name.clone(),
        }
    }
}

/// Output structure for tab information in list/info commands
#[derive(Debug, Clone, Serialize)]
pub struct TabInfoOutput {
    pub tab_name: String,
    pub session: String,
    pub correlation_id: Option<String>,
    pub created_at: String,
    pub last_accessed: String,
    pub meta: HashMap<String, String>,
    pub pane_count: usize,
}

// ============================================================================
// Pane Tracking Types (Perth v1.0 - Legacy)
// ============================================================================

#[derive(Debug, Clone)]
pub struct PaneRecord {
    pub pane_name: String,
    pub session: String,
    pub tab: String,
    pub pane_id: Option<String>,
    pub created_at: String,
    pub last_seen: String,
    pub last_accessed: String,
    pub meta: HashMap<String, String>,
    pub stale: bool,
}

impl PaneRecord {
    pub fn new(
        pane_name: String,
        session: String,
        tab: String,
        now: String,
        meta: HashMap<String, String>,
    ) -> Self {
        Self {
            pane_name,
            session,
            tab,
            pane_id: None,
            created_at: now.clone(),
            last_seen: now.clone(),
            last_accessed: now,
            meta,
            stale: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PaneStatus {
    Found,
    Stale,
    Missing,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaneInfoOutput {
    pub pane_name: String,
    pub session: String,
    pub tab: String,
    pub pane_id: Option<String>,
    pub created_at: String,
    pub last_seen: String,
    pub last_accessed: String,
    pub meta: HashMap<String, String>,
    pub status: PaneStatus,
    pub source: String,
}

impl PaneInfoOutput {
    pub fn missing(pane_name: String) -> Self {
        Self {
            pane_name,
            session: String::new(),
            tab: String::new(),
            pane_id: None,
            created_at: String::new(),
            last_seen: String::new(),
            last_accessed: String::new(),
            meta: HashMap::new(),
            status: PaneStatus::Missing,
            source: "redis".to_string(),
        }
    }
}

// ============================================================================
// Session Restoration Types (Perth v2.1 - STORY-040)
// ============================================================================

/// Snapshot of a single pane's state for restoration.
///
/// Captures all information needed to recreate a pane, including
/// its position, working directory, and running command.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneSnapshot {
    /// Pane name (used for identification)
    pub name: String,
    /// Position index within the tab
    pub position: usize,
    /// Working directory when snapshot was taken
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    /// Command running in the pane (if detectable)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    /// Zellij pane ID (for reference, may change on restore)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pane_id: Option<String>,
    /// Whether pane was focused when snapshot was taken
    #[serde(default)]
    pub focused: bool,
    /// Additional metadata from Perth tracking
    #[serde(default)]
    pub meta: HashMap<String, String>,
}

/// Snapshot of a tab's state including all panes.
///
/// Captures tab layout and pane configuration for restoration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TabSnapshot {
    /// Tab name
    pub name: String,
    /// Tab index in the session
    pub index: usize,
    /// Whether tab was active when snapshot was taken
    #[serde(default)]
    pub active: bool,
    /// Layout direction (vertical/horizontal)
    #[serde(default)]
    pub layout: String,
    /// Panes within this tab, ordered by position
    pub panes: Vec<PaneSnapshot>,
    /// Correlation ID if tab was created with one
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

/// Complete snapshot of a Zellij session.
///
/// This is the top-level structure stored in Redis for restoration.
/// Redis key format: `perth:snapshots:{session}:{name}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSnapshot {
    /// Snapshot schema version for forward compatibility
    pub schema_version: String,
    /// Unique identifier for this snapshot
    pub id: Uuid,
    /// Human-readable name for this snapshot
    pub name: String,
    /// Session name this snapshot belongs to
    pub session: String,
    /// When snapshot was created
    pub created_at: DateTime<Utc>,
    /// Optional description or notes
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Parent snapshot ID for incremental snapshots
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<Uuid>,
    /// Tabs in this session, ordered by index
    pub tabs: Vec<TabSnapshot>,
    /// Total pane count for quick reference
    pub pane_count: usize,
}

impl SessionSnapshot {
    /// Create a new session snapshot
    pub fn new(name: impl Into<String>, session: impl Into<String>) -> Self {
        Self {
            schema_version: "1.0".to_string(),
            id: Uuid::new_v4(),
            name: name.into(),
            session: session.into(),
            created_at: Utc::now(),
            description: None,
            parent_id: None,
            tabs: Vec::new(),
            pane_count: 0,
        }
    }

    /// Builder method to set description
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = Some(desc.into());
        self
    }

    /// Builder method to set parent for incremental snapshot
    pub fn with_parent(mut self, parent_id: Uuid) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Add a tab to the snapshot
    pub fn add_tab(&mut self, tab: TabSnapshot) {
        self.pane_count += tab.panes.len();
        self.tabs.push(tab);
    }

    /// Get Redis key for this snapshot
    pub fn redis_key(&self) -> String {
        format!("perth:snapshots:{}:{}", self.session, self.name)
    }
}

/// Warning level for restoration issues.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RestoreWarningLevel {
    /// Informational, restoration succeeded
    Info,
    /// Minor issue, restoration partially succeeded
    Warning,
    /// Serious issue, part of restoration failed
    Error,
}

/// Individual warning or issue encountered during restoration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreWarning {
    /// Severity level
    pub level: RestoreWarningLevel,
    /// Human-readable message
    pub message: String,
    /// Component that had the issue (tab name, pane name, etc.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub component: Option<String>,
    /// Suggested remediation if applicable
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

impl RestoreWarning {
    /// Create an info-level warning
    pub fn info(message: impl Into<String>) -> Self {
        Self {
            level: RestoreWarningLevel::Info,
            message: message.into(),
            component: None,
            suggestion: None,
        }
    }

    /// Create a warning-level warning
    pub fn warning(message: impl Into<String>) -> Self {
        Self {
            level: RestoreWarningLevel::Warning,
            message: message.into(),
            component: None,
            suggestion: None,
        }
    }

    /// Create an error-level warning
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            level: RestoreWarningLevel::Error,
            message: message.into(),
            component: None,
            suggestion: None,
        }
    }

    /// Add component context
    pub fn for_component(mut self, component: impl Into<String>) -> Self {
        self.component = Some(component.into());
        self
    }

    /// Add remediation suggestion
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }
}

/// Overall restoration status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RestoreStatus {
    /// All components restored successfully
    Success,
    /// Restored with some warnings
    Partial,
    /// Restoration failed
    Failed,
}

/// Result of a restoration attempt.
///
/// Provides detailed feedback about what was restored and any issues.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestoreReport {
    /// Overall status
    pub status: RestoreStatus,
    /// Snapshot that was restored
    pub snapshot_name: String,
    /// Session that was restored to
    pub session: String,
    /// Number of tabs restored
    pub tabs_restored: usize,
    /// Number of panes restored
    pub panes_restored: usize,
    /// Number of tabs that failed to restore
    pub tabs_failed: usize,
    /// Number of panes that failed to restore
    pub panes_failed: usize,
    /// When restoration was performed
    pub restored_at: DateTime<Utc>,
    /// Duration of restoration in milliseconds
    pub duration_ms: u64,
    /// Warnings and issues encountered
    #[serde(default)]
    pub warnings: Vec<RestoreWarning>,
}

impl RestoreReport {
    /// Create a new restore report
    pub fn new(snapshot_name: impl Into<String>, session: impl Into<String>) -> Self {
        Self {
            status: RestoreStatus::Success,
            snapshot_name: snapshot_name.into(),
            session: session.into(),
            tabs_restored: 0,
            panes_restored: 0,
            tabs_failed: 0,
            panes_failed: 0,
            restored_at: Utc::now(),
            duration_ms: 0,
            warnings: Vec::new(),
        }
    }

    /// Add a warning to the report
    pub fn add_warning(&mut self, warning: RestoreWarning) {
        // Update status based on warning level
        match warning.level {
            RestoreWarningLevel::Error => {
                self.status = RestoreStatus::Failed;
            }
            RestoreWarningLevel::Warning if self.status == RestoreStatus::Success => {
                self.status = RestoreStatus::Partial;
            }
            _ => {}
        }
        self.warnings.push(warning);
    }

    /// Check if restoration was fully successful
    pub fn is_success(&self) -> bool {
        self.status == RestoreStatus::Success
    }

    /// Get count of errors
    pub fn error_count(&self) -> usize {
        self.warnings
            .iter()
            .filter(|w| w.level == RestoreWarningLevel::Error)
            .count()
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_intent_entry_serialization_roundtrip() {
        let entry = IntentEntry::new("Implementing STORY-001")
            .with_type(IntentType::Milestone)
            .with_artifacts(vec!["src/types.rs".to_string(), "Cargo.toml".to_string()])
            .with_source(IntentSource::Manual)
            .with_goal_delta("Added IntentEntry data model")
            .with_commands_run(5);

        // Serialize to JSON
        let json = serde_json::to_string(&entry).expect("Failed to serialize IntentEntry");

        // Deserialize back
        let deserialized: IntentEntry =
            serde_json::from_str(&json).expect("Failed to deserialize IntentEntry");

        // Verify all fields match
        assert_eq!(entry.id, deserialized.id);
        assert_eq!(entry.timestamp, deserialized.timestamp);
        assert_eq!(entry.summary, deserialized.summary);
        assert_eq!(entry.entry_type, deserialized.entry_type);
        assert_eq!(entry.artifacts, deserialized.artifacts);
        assert_eq!(entry.commands_run, deserialized.commands_run);
        assert_eq!(entry.goal_delta, deserialized.goal_delta);
        assert_eq!(entry.source, deserialized.source);
    }

    #[test]
    fn test_intent_entry_minimal_serialization() {
        // Test with only required fields (defaults for optional)
        let entry = IntentEntry::new("Quick checkpoint");

        let json = serde_json::to_string(&entry).expect("Failed to serialize");
        let deserialized: IntentEntry = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(entry.summary, deserialized.summary);
        assert_eq!(deserialized.entry_type, IntentType::Checkpoint); // default
        assert_eq!(deserialized.source, IntentSource::Manual); // default
        assert!(deserialized.artifacts.is_empty());
        assert!(deserialized.commands_run.is_none());
        assert!(deserialized.goal_delta.is_none());
    }

    #[test]
    fn test_intent_type_serialization() {
        // Test enum serialization with lowercase
        assert_eq!(
            serde_json::to_string(&IntentType::Milestone).unwrap(),
            "\"milestone\""
        );
        assert_eq!(
            serde_json::to_string(&IntentType::Checkpoint).unwrap(),
            "\"checkpoint\""
        );
        assert_eq!(
            serde_json::to_string(&IntentType::Exploration).unwrap(),
            "\"exploration\""
        );

        // Test deserialization
        assert_eq!(
            serde_json::from_str::<IntentType>("\"milestone\"").unwrap(),
            IntentType::Milestone
        );
    }

    #[test]
    fn test_intent_source_serialization() {
        // Test enum serialization with lowercase
        assert_eq!(
            serde_json::to_string(&IntentSource::Manual).unwrap(),
            "\"manual\""
        );
        assert_eq!(
            serde_json::to_string(&IntentSource::Automated).unwrap(),
            "\"automated\""
        );
        assert_eq!(
            serde_json::to_string(&IntentSource::Agent).unwrap(),
            "\"agent\""
        );
    }

    #[test]
    fn test_intent_entry_json_structure() {
        let entry = IntentEntry::new("Test entry")
            .with_type(IntentType::Exploration)
            .with_source(IntentSource::Agent);

        let json = serde_json::to_string_pretty(&entry).unwrap();

        // Verify JSON contains expected keys
        assert!(json.contains("\"id\""));
        assert!(json.contains("\"timestamp\""));
        assert!(json.contains("\"summary\""));
        assert!(json.contains("\"entry_type\""));
        assert!(json.contains("\"exploration\"")); // lowercase enum
        assert!(json.contains("\"source\""));
        assert!(json.contains("\"agent\"")); // lowercase enum

        // Optional fields with None should not appear (skip_serializing_if)
        assert!(!json.contains("\"commands_run\""));
        assert!(!json.contains("\"goal_delta\""));
    }

    #[test]
    fn test_intent_entry_builder_pattern() {
        let entry = IntentEntry::new("Building feature")
            .with_type(IntentType::Milestone)
            .with_artifacts(vec!["file1.rs".to_string()])
            .with_source(IntentSource::Automated)
            .with_goal_delta("Completed implementation")
            .with_commands_run(10);

        assert_eq!(entry.summary, "Building feature");
        assert_eq!(entry.entry_type, IntentType::Milestone);
        assert_eq!(entry.artifacts, vec!["file1.rs"]);
        assert_eq!(entry.source, IntentSource::Automated);
        assert_eq!(entry.goal_delta, Some("Completed implementation".to_string()));
        assert_eq!(entry.commands_run, Some(10));
    }

    // ========================================================================
    // TabRecord Tests (STORY-036)
    // ========================================================================

    #[test]
    fn test_tab_record_serialization_roundtrip() {
        let tab = TabRecord::new(
            "myapp(fixes)".to_string(),
            "main".to_string(),
            "2026-01-07T12:00:00Z".to_string(),
        )
        .with_correlation_id("pr-42");

        // Serialize to JSON
        let json = serde_json::to_string(&tab).expect("Failed to serialize TabRecord");

        // Deserialize back
        let deserialized: TabRecord =
            serde_json::from_str(&json).expect("Failed to deserialize TabRecord");

        // Verify all fields match
        assert_eq!(tab.tab_name, deserialized.tab_name);
        assert_eq!(tab.session, deserialized.session);
        assert_eq!(tab.correlation_id, deserialized.correlation_id);
        assert_eq!(tab.created_at, deserialized.created_at);
        assert_eq!(tab.last_accessed, deserialized.last_accessed);
    }

    #[test]
    fn test_tab_record_without_correlation_id() {
        let tab = TabRecord::new(
            "simple-tab".to_string(),
            "main".to_string(),
            "2026-01-07T12:00:00Z".to_string(),
        );

        // Serialize to JSON
        let json = serde_json::to_string(&tab).expect("Failed to serialize");

        // correlation_id should be omitted when None (skip_serializing_if)
        assert!(!json.contains("correlation_id"));

        // Deserialize back
        let deserialized: TabRecord = serde_json::from_str(&json).expect("Failed to deserialize");
        assert!(deserialized.correlation_id.is_none());
    }

    #[test]
    fn test_tab_record_effective_name() {
        // Without correlation ID
        let tab1 = TabRecord::new(
            "myapp(fixes)".to_string(),
            "main".to_string(),
            "2026-01-07T12:00:00Z".to_string(),
        );
        assert_eq!(tab1.effective_name(), "myapp(fixes)");

        // With correlation ID
        let tab2 = TabRecord::new(
            "myapp(fixes)".to_string(),
            "main".to_string(),
            "2026-01-07T12:00:00Z".to_string(),
        )
        .with_correlation_id("pr-42");
        assert_eq!(tab2.effective_name(), "myapp(fixes)-pr-42");
    }

    #[test]
    fn test_tab_record_with_meta() {
        let mut meta = HashMap::new();
        meta.insert("project".to_string(), "perth".to_string());
        meta.insert("priority".to_string(), "high".to_string());

        let tab = TabRecord::new(
            "myapp(fixes)".to_string(),
            "main".to_string(),
            "2026-01-07T12:00:00Z".to_string(),
        )
        .with_correlation_id("abc123")
        .with_meta(meta);

        let json = serde_json::to_string(&tab).expect("Failed to serialize");
        let deserialized: TabRecord = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(deserialized.meta.get("project"), Some(&"perth".to_string()));
        assert_eq!(deserialized.meta.get("priority"), Some(&"high".to_string()));
    }

    // ========================================================================
    // Session Restoration Tests (STORY-040)
    // ========================================================================

    #[test]
    fn test_session_snapshot_serialization_roundtrip() {
        let mut snapshot = SessionSnapshot::new("pre-refactor", "main")
            .with_description("Snapshot before major refactor");

        // Add a tab with panes
        let pane1 = PaneSnapshot {
            name: "editor".to_string(),
            position: 0,
            cwd: Some("/home/user/project".to_string()),
            command: Some("nvim".to_string()),
            pane_id: Some("1".to_string()),
            focused: true,
            meta: HashMap::new(),
        };

        let pane2 = PaneSnapshot {
            name: "terminal".to_string(),
            position: 1,
            cwd: Some("/home/user/project".to_string()),
            command: None,
            pane_id: Some("2".to_string()),
            focused: false,
            meta: HashMap::new(),
        };

        let tab = TabSnapshot {
            name: "myapp(dev)".to_string(),
            index: 0,
            active: true,
            layout: "vertical".to_string(),
            panes: vec![pane1, pane2],
            correlation_id: Some("pr-42".to_string()),
        };

        snapshot.add_tab(tab);

        // Serialize to JSON
        let json = serde_json::to_string(&snapshot).expect("Failed to serialize SessionSnapshot");

        // Deserialize back
        let deserialized: SessionSnapshot =
            serde_json::from_str(&json).expect("Failed to deserialize SessionSnapshot");

        // Verify fields
        assert_eq!(snapshot.id, deserialized.id);
        assert_eq!(snapshot.name, deserialized.name);
        assert_eq!(snapshot.session, deserialized.session);
        assert_eq!(snapshot.schema_version, "1.0");
        assert_eq!(deserialized.description, Some("Snapshot before major refactor".to_string()));
        assert_eq!(deserialized.tabs.len(), 1);
        assert_eq!(deserialized.pane_count, 2);
        assert_eq!(deserialized.tabs[0].panes.len(), 2);
        assert_eq!(deserialized.tabs[0].correlation_id, Some("pr-42".to_string()));
    }

    #[test]
    fn test_session_snapshot_redis_key() {
        let snapshot = SessionSnapshot::new("backup-v1", "my-session");
        assert_eq!(snapshot.redis_key(), "perth:snapshots:my-session:backup-v1");
    }

    #[test]
    fn test_restore_warning_builder() {
        let warning = RestoreWarning::warning("Pane cwd no longer exists")
            .for_component("editor")
            .with_suggestion("Will use current directory instead");

        assert_eq!(warning.level, RestoreWarningLevel::Warning);
        assert_eq!(warning.message, "Pane cwd no longer exists");
        assert_eq!(warning.component, Some("editor".to_string()));
        assert_eq!(warning.suggestion, Some("Will use current directory instead".to_string()));
    }

    #[test]
    fn test_restore_report_status_updates() {
        let mut report = RestoreReport::new("test-snapshot", "main");
        assert_eq!(report.status, RestoreStatus::Success);
        assert!(report.is_success());

        // Add info warning - should stay Success
        report.add_warning(RestoreWarning::info("Tab restored"));
        assert_eq!(report.status, RestoreStatus::Success);

        // Add warning - should become Partial
        report.add_warning(RestoreWarning::warning("Pane position adjusted"));
        assert_eq!(report.status, RestoreStatus::Partial);

        // Add error - should become Failed
        report.add_warning(RestoreWarning::error("Failed to restore pane"));
        assert_eq!(report.status, RestoreStatus::Failed);
        assert!(!report.is_success());
        assert_eq!(report.error_count(), 1);
    }

    #[test]
    fn test_restore_report_serialization() {
        let mut report = RestoreReport::new("snapshot-1", "dev-session");
        report.tabs_restored = 2;
        report.panes_restored = 5;
        report.add_warning(RestoreWarning::warning("Minor issue").for_component("pane-3"));

        let json = serde_json::to_string(&report).expect("Failed to serialize");
        let deserialized: RestoreReport = serde_json::from_str(&json).expect("Failed to deserialize");

        assert_eq!(deserialized.snapshot_name, "snapshot-1");
        assert_eq!(deserialized.session, "dev-session");
        assert_eq!(deserialized.tabs_restored, 2);
        assert_eq!(deserialized.panes_restored, 5);
        assert_eq!(deserialized.warnings.len(), 1);
        assert_eq!(deserialized.status, RestoreStatus::Partial);
    }

    #[test]
    fn test_pane_snapshot_optional_fields() {
        // Minimal pane snapshot
        let pane = PaneSnapshot {
            name: "minimal".to_string(),
            position: 0,
            cwd: None,
            command: None,
            pane_id: None,
            focused: false,
            meta: HashMap::new(),
        };

        let json = serde_json::to_string(&pane).expect("Failed to serialize");

        // Optional fields should be omitted
        assert!(!json.contains("cwd"));
        assert!(!json.contains("command"));
        assert!(!json.contains("pane_id"));

        // Deserialize and verify
        let deserialized: PaneSnapshot = serde_json::from_str(&json).expect("Failed to deserialize");
        assert_eq!(deserialized.name, "minimal");
        assert!(deserialized.cwd.is_none());
    }
}
