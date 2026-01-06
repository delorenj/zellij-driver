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
}
