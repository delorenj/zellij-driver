//! Bloodbank Event Publisher for Perth (STORY-025)
//!
//! Publishes events to RabbitMQ for integration with the 33GOD ecosystem.
//! Events follow the Bloodbank naming convention: `<source>.<entity>.<past-tense-action>`
//!
//! Perth events:
//! - `perth.pane.created` - A new pane was created
//! - `perth.pane.opened` - An existing pane was opened/resumed
//! - `perth.tab.created` - A new tab was created
//! - `perth.intent.logged` - An intent entry was logged
//! - `perth.milestone.recorded` - A milestone was recorded (intent with type=milestone)

use crate::config::BloodbankConfig;
use crate::types::{IntentEntry, IntentType, PaneRecord, TabRecord};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use lapin::{
    options::{BasicPublishOptions, ExchangeDeclareOptions},
    types::FieldTable,
    BasicProperties, Channel, Connection, ConnectionProperties, ExchangeKind,
};
use serde::Serialize;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Event envelope wrapping all Bloodbank events
#[derive(Debug, Clone, Serialize)]
pub struct EventEnvelope<T: Serialize> {
    /// Event type following Bloodbank naming: source.entity.action
    pub event_type: String,
    /// ISO 8601 timestamp
    pub timestamp: DateTime<Utc>,
    /// Event payload
    pub payload: T,
    /// Event metadata
    pub metadata: EventMetadata,
}

/// Metadata attached to every event
#[derive(Debug, Clone, Serialize)]
pub struct EventMetadata {
    /// Source system identifier
    pub source: String,
    /// Source version
    pub version: String,
    /// Optional correlation ID for tracing
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
    /// Optional session name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

impl Default for EventMetadata {
    fn default() -> Self {
        Self {
            source: "perth".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            correlation_id: None,
            session: None,
        }
    }
}

impl EventMetadata {
    pub fn with_correlation_id(mut self, id: impl Into<String>) -> Self {
        self.correlation_id = Some(id.into());
        self
    }

    pub fn with_session(mut self, session: impl Into<String>) -> Self {
        self.session = Some(session.into());
        self
    }
}

// ============================================================================
// Event Payloads
// ============================================================================

/// Payload for pane.created event
#[derive(Debug, Clone, Serialize)]
pub struct PaneCreatedPayload {
    pub pane_name: String,
    pub tab: String,
    pub session: String,
    pub position: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
}

impl From<&PaneRecord> for PaneCreatedPayload {
    fn from(record: &PaneRecord) -> Self {
        Self {
            pane_name: record.pane_name.clone(),
            tab: record.tab.clone(),
            session: record.session.clone(),
            position: record.meta.get("position").and_then(|p| p.parse().ok()),
            cwd: record.meta.get("cwd").cloned(),
        }
    }
}

/// Payload for pane.opened event
#[derive(Debug, Clone, Serialize)]
pub struct PaneOpenedPayload {
    pub pane_name: String,
    pub tab: String,
    pub session: String,
}

/// Payload for tab.created event
#[derive(Debug, Clone, Serialize)]
pub struct TabCreatedPayload {
    pub tab_name: String,
    pub session: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

impl From<&TabRecord> for TabCreatedPayload {
    fn from(record: &TabRecord) -> Self {
        Self {
            tab_name: record.tab_name.clone(),
            session: record.session.clone(),
            correlation_id: record.correlation_id.clone(),
        }
    }
}

/// Payload for intent.logged event
#[derive(Debug, Clone, Serialize)]
pub struct IntentLoggedPayload {
    pub pane_name: String,
    pub intent_id: String,
    pub summary: String,
    pub entry_type: String,
    pub source: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<String>,
}

impl IntentLoggedPayload {
    pub fn new(pane_name: &str, entry: &IntentEntry) -> Self {
        Self {
            pane_name: pane_name.to_string(),
            intent_id: entry.id.to_string(),
            summary: entry.summary.clone(),
            entry_type: entry.entry_type_str().to_lowercase(),
            source: entry.source_str().to_string(),
            artifacts: entry.artifacts.clone(),
        }
    }
}

/// Payload for milestone.recorded event (special case of intent.logged)
#[derive(Debug, Clone, Serialize)]
pub struct MilestoneRecordedPayload {
    pub pane_name: String,
    pub intent_id: String,
    pub summary: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub artifacts: Vec<String>,
}

impl MilestoneRecordedPayload {
    pub fn new(pane_name: &str, entry: &IntentEntry) -> Self {
        Self {
            pane_name: pane_name.to_string(),
            intent_id: entry.id.to_string(),
            summary: entry.summary.clone(),
            artifacts: entry.artifacts.clone(),
        }
    }
}

// ============================================================================
// Event Publisher
// ============================================================================

/// Connection state for the event publisher
enum ConnectionState {
    /// Not connected, will attempt on next publish
    Disconnected,
    /// Connected and ready to publish
    Connected(Channel),
    /// Disabled (config.enabled = false)
    Disabled,
}

/// Publisher for Bloodbank events via RabbitMQ
pub struct EventPublisher {
    config: BloodbankConfig,
    state: Arc<RwLock<ConnectionState>>,
}

impl EventPublisher {
    /// Create a new event publisher with the given configuration
    pub fn new(config: BloodbankConfig) -> Self {
        let initial_state = if config.enabled {
            ConnectionState::Disconnected
        } else {
            ConnectionState::Disabled
        };

        Self {
            config,
            state: Arc::new(RwLock::new(initial_state)),
        }
    }

    /// Check if publishing is enabled
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Attempt to connect to RabbitMQ
    async fn connect(&self) -> Result<Channel> {
        let conn = Connection::connect(&self.config.amqp_url, ConnectionProperties::default())
            .await
            .context("failed to connect to RabbitMQ")?;

        let channel = conn.create_channel().await.context("failed to create channel")?;

        // Declare the exchange (topic type for routing key patterns)
        channel
            .exchange_declare(
                &self.config.exchange,
                ExchangeKind::Topic,
                ExchangeDeclareOptions {
                    durable: true,
                    ..Default::default()
                },
                FieldTable::default(),
            )
            .await
            .context("failed to declare exchange")?;

        Ok(channel)
    }

    /// Get or create a channel for publishing
    async fn get_channel(&self) -> Result<Channel> {
        // Check current state
        {
            let state = self.state.read().await;
            match &*state {
                ConnectionState::Disabled => {
                    return Err(anyhow::anyhow!("Bloodbank publishing is disabled"));
                }
                ConnectionState::Connected(channel) => {
                    if channel.status().connected() {
                        return Ok(channel.clone());
                    }
                    // Channel disconnected, fall through to reconnect
                }
                ConnectionState::Disconnected => {
                    // Fall through to connect
                }
            }
        }

        // Need to connect/reconnect
        let channel = self.connect().await?;

        // Update state
        {
            let mut state = self.state.write().await;
            *state = ConnectionState::Connected(channel.clone());
        }

        Ok(channel)
    }

    /// Publish an event to Bloodbank
    ///
    /// This method handles connection failures gracefully - if RabbitMQ is
    /// unavailable, it logs a warning but does not return an error.
    pub async fn publish<T: Serialize>(&self, event_type: &str, payload: T, metadata: EventMetadata) {
        if !self.config.enabled {
            return;
        }

        let envelope = EventEnvelope {
            event_type: event_type.to_string(),
            timestamp: Utc::now(),
            payload,
            metadata,
        };

        // Build routing key: perth.pane.created -> perth.pane.created
        let routing_key = event_type;

        let body = match serde_json::to_vec(&envelope) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("Warning: Failed to serialize event {}: {}", event_type, e);
                return;
            }
        };

        let channel = match self.get_channel().await {
            Ok(c) => c,
            Err(e) => {
                // Graceful degradation: log warning but don't fail
                eprintln!("Warning: Bloodbank unavailable, event {} not published: {}", event_type, e);
                return;
            }
        };

        let props = BasicProperties::default()
            .with_content_type("application/json".into())
            .with_delivery_mode(2); // Persistent

        if let Err(e) = channel
            .basic_publish(
                &self.config.exchange,
                routing_key,
                BasicPublishOptions::default(),
                &body,
                props,
            )
            .await
        {
            eprintln!("Warning: Failed to publish event {}: {}", event_type, e);
        }
    }

    // ========================================================================
    // Convenience methods for specific events
    // ========================================================================

    /// Publish pane.created event
    pub async fn pane_created(&self, record: &PaneRecord) {
        let payload = PaneCreatedPayload::from(record);
        let metadata = EventMetadata::default().with_session(&record.session);
        self.publish("perth.pane.created", payload, metadata).await;
    }

    /// Publish pane.opened event
    pub async fn pane_opened(&self, pane_name: &str, tab: &str, session: &str) {
        let payload = PaneOpenedPayload {
            pane_name: pane_name.to_string(),
            tab: tab.to_string(),
            session: session.to_string(),
        };
        let metadata = EventMetadata::default().with_session(session);
        self.publish("perth.pane.opened", payload, metadata).await;
    }

    /// Publish tab.created event
    pub async fn tab_created(&self, record: &TabRecord) {
        let payload = TabCreatedPayload::from(record);
        let mut metadata = EventMetadata::default().with_session(&record.session);
        if let Some(ref cid) = record.correlation_id {
            metadata = metadata.with_correlation_id(cid);
        }
        self.publish("perth.tab.created", payload, metadata).await;
    }

    /// Publish intent.logged event
    pub async fn intent_logged(&self, pane_name: &str, entry: &IntentEntry, session: Option<&str>) {
        let payload = IntentLoggedPayload::new(pane_name, entry);
        let mut metadata = EventMetadata::default();
        if let Some(s) = session {
            metadata = metadata.with_session(s);
        }
        self.publish("perth.intent.logged", payload, metadata.clone()).await;

        // If it's a milestone, also publish the milestone.recorded event
        if entry.entry_type == IntentType::Milestone {
            let milestone_payload = MilestoneRecordedPayload::new(pane_name, entry);
            self.publish("perth.milestone.recorded", milestone_payload, metadata).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::IntentSource;

    #[test]
    fn test_event_envelope_serialization() {
        let payload = PaneCreatedPayload {
            pane_name: "test-pane".to_string(),
            tab: "test-tab".to_string(),
            session: "test-session".to_string(),
            position: Some(0),
            cwd: None,
        };

        let envelope = EventEnvelope {
            event_type: "perth.pane.created".to_string(),
            timestamp: Utc::now(),
            payload,
            metadata: EventMetadata::default(),
        };

        let json = serde_json::to_string(&envelope).unwrap();
        assert!(json.contains("perth.pane.created"));
        assert!(json.contains("test-pane"));
    }

    #[test]
    fn test_intent_logged_payload() {
        let entry = IntentEntry::new("Test milestone")
            .with_type(IntentType::Milestone)
            .with_source(IntentSource::Agent);

        let payload = IntentLoggedPayload::new("test-pane", &entry);

        assert_eq!(payload.pane_name, "test-pane");
        assert_eq!(payload.summary, "Test milestone");
        assert_eq!(payload.entry_type, "milestone");
        assert_eq!(payload.source, "agent");
    }

    #[test]
    fn test_metadata_builder() {
        let metadata = EventMetadata::default()
            .with_correlation_id("abc123")
            .with_session("my-session");

        assert_eq!(metadata.correlation_id, Some("abc123".to_string()));
        assert_eq!(metadata.session, Some("my-session".to_string()));
        assert_eq!(metadata.source, "perth");
    }

    #[test]
    fn test_publisher_disabled() {
        let config = BloodbankConfig {
            enabled: false,
            ..Default::default()
        };

        let publisher = EventPublisher::new(config);
        assert!(!publisher.is_enabled());
    }
}
