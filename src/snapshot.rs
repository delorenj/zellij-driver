use crate::types::{PaneSnapshot, RestoreReport, RestoreWarning, SessionSnapshot, TabSnapshot};
use crate::zellij::ZellijDriver;
use anyhow::{anyhow, Result};
use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// State capture module for creating session snapshots.
///
/// This module interfaces with Zellij to capture the current session state
/// (tabs, panes, layout) and transforms it into our restoration data model.
pub struct StateCapture {
    zellij: ZellijDriver,
}

impl StateCapture {
    pub fn new(zellij: ZellijDriver) -> Self {
        Self { zellij }
    }

    /// Capture the current session state and create a snapshot.
    ///
    /// # Arguments
    /// * `name` - User-friendly name for this snapshot
    /// * `description` - Optional description of what this snapshot captures
    /// * `parent_id` - Optional parent snapshot ID for incremental snapshots
    ///
    /// # Returns
    /// A tuple of (SessionSnapshot, RestoreReport) where the report contains
    /// any warnings encountered during capture.
    pub async fn capture_session(
        &self,
        name: String,
        description: Option<String>,
        parent_id: Option<Uuid>,
    ) -> Result<(SessionSnapshot, RestoreReport)> {
        // Get active session name from environment
        let session = self
            .zellij
            .active_session_name()
            .ok_or_else(|| anyhow!("not inside a zellij session; snapshot requires active session"))?;

        // Query Zellij layout
        let layout = self
            .zellij
            .dump_layout_json(Some(&session))
            .await?
            .ok_or_else(|| anyhow!("failed to get layout from zellij; dump-layout returned empty"))?;

        // Initialize report for tracking warnings
        let mut report = RestoreReport::new(name.clone(), session.clone());

        // Parse tabs from layout
        let tabs = self.parse_tabs(&layout, &mut report).await?;

        // Calculate total pane count
        let pane_count = tabs.iter().map(|t| t.panes.len()).sum();

        // Build snapshot
        let snapshot = SessionSnapshot {
            schema_version: "1.0.0".to_string(),
            id: Uuid::new_v4(),
            name: name.clone(),
            session: session.clone(),
            created_at: Utc::now(),
            description,
            parent_id,
            tabs,
            pane_count,
        };

        // Status is automatically updated by add_warning() calls

        Ok((snapshot, report))
    }

    /// Parse tabs from Zellij layout JSON.
    async fn parse_tabs(&self, layout: &Value, report: &mut RestoreReport) -> Result<Vec<TabSnapshot>> {
        let tabs_array = layout
            .get("tabs")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("layout missing 'tabs' array"))?;

        let mut tabs = Vec::new();

        for (index, tab_value) in tabs_array.iter().enumerate() {
            match self.parse_tab(tab_value, index, report).await {
                Ok(tab) => tabs.push(tab),
                Err(e) => {
                    let warning = RestoreWarning::warning(format!("failed to parse tab: {}", e))
                        .for_component(format!("tab at index {}", index));
                    report.add_warning(warning);
                }
            }
        }

        if tabs.is_empty() {
            return Err(anyhow!("no tabs captured; session appears empty"));
        }

        Ok(tabs)
    }

    /// Parse a single tab from Zellij layout JSON.
    async fn parse_tab(
        &self,
        tab_value: &Value,
        index: usize,
        report: &mut RestoreReport,
    ) -> Result<TabSnapshot> {
        let tab_obj = tab_value
            .as_object()
            .ok_or_else(|| anyhow!("tab is not an object"))?;

        // Extract tab name (required)
        let name = tab_obj
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("tab missing 'name' field"))?
            .to_string();

        // Parse panes from this tab
        let panes = self.parse_panes(tab_obj, &name, report)?;

        // Extract layout direction (vertical/horizontal, default to vertical)
        let layout = tab_obj
            .get("layout")
            .and_then(|v| v.as_str())
            .unwrap_or("vertical")
            .to_string();

        // Extract active state (whether this tab is currently focused)
        let active = tab_obj
            .get("active")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        Ok(TabSnapshot {
            name,
            index,
            active,
            layout,
            panes,
            correlation_id: None, // Will be populated from Redis metadata if available
        })
    }

    /// Parse panes from a tab object.
    ///
    /// Flattens nested pane structures and assigns position indices.
    fn parse_panes(
        &self,
        tab_obj: &serde_json::Map<String, Value>,
        tab_name: &str,
        report: &mut RestoreReport,
    ) -> Result<Vec<PaneSnapshot>> {
        let mut panes = Vec::new();
        let mut position = 0;

        // Parse tiled panes (recursively flatten splits)
        if let Some(panes_array) = tab_obj.get("panes").and_then(|v| v.as_array()) {
            for pane_value in panes_array {
                self.collect_panes(pane_value, tab_name, &mut panes, &mut position, report);
            }
        }

        // Parse floating panes
        if let Some(floating_array) = tab_obj.get("floating_panes").and_then(|v| v.as_array()) {
            for pane_value in floating_array {
                self.collect_panes(pane_value, tab_name, &mut panes, &mut position, report);
            }
        }

        Ok(panes)
    }

    /// Recursively collect panes from layout JSON, flattening splits.
    ///
    /// This handles nested pane structures (splits) by recursively traversing
    /// and assigning sequential position indices to all leaf panes.
    fn collect_panes(
        &self,
        pane_value: &Value,
        tab_name: &str,
        panes: &mut Vec<PaneSnapshot>,
        position: &mut usize,
        report: &mut RestoreReport,
    ) {
        let Some(pane_obj) = pane_value.as_object() else {
            return;
        };

        // If this is a split pane (contains nested panes), recurse
        if let Some(nested_panes) = pane_obj.get("panes").and_then(|v| v.as_array()) {
            for nested_pane in nested_panes {
                self.collect_panes(nested_pane, tab_name, panes, position, report);
            }
            return;
        }

        // Leaf pane - extract info
        let name = pane_obj
            .get("pane_name")
            .and_then(|v| v.as_str())
            .or_else(|| pane_obj.get("name").and_then(|v| v.as_str()))
            .unwrap_or("unnamed")
            .to_string();

        let cwd = pane_obj
            .get("cwd")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let command = pane_obj
            .get("command")
            .and_then(|v| v.as_str())
            .or_else(|| pane_obj.get("running_command").and_then(|v| v.as_str()))
            .map(|s| s.to_string());

        let pane_id = pane_obj
            .get("id")
            .and_then(|v| v.as_u64())
            .or_else(|| pane_obj.get("pane_id").and_then(|v| v.as_u64()))
            .map(|n| n.to_string());

        let focused = pane_obj
            .get("focused")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Warn if unnamed pane
        if name == "unnamed" {
            let warning = RestoreWarning::info("pane has no name; will be restored as unnamed")
                .for_component(format!("tab '{}' position {}", tab_name, position));
            report.add_warning(warning);
        }

        panes.push(PaneSnapshot {
            name,
            position: *position,
            cwd,
            command,
            pane_id,
            focused,
            meta: HashMap::new(), // Will be populated from Redis if pane is tracked
        });

        *position += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_snapshot_redis_key_generation() {
        let snapshot = SessionSnapshot {
            schema_version: "1.0.0".to_string(),
            id: Uuid::new_v4(),
            name: "my-snapshot".to_string(),
            session: "dev-session".to_string(),
            created_at: Utc::now(),
            description: None,
            parent_id: None,
            tabs: vec![],
            pane_count: 0,
        };

        assert_eq!(
            snapshot.redis_key(),
            "perth:snapshots:dev-session:my-snapshot"
        );
    }

    #[test]
    fn test_session_snapshot_builder() {
        let snapshot = SessionSnapshot::new("test-snap", "my-session")
            .with_description("Test snapshot for unit test");

        assert_eq!(snapshot.name, "test-snap");
        assert_eq!(snapshot.session, "my-session");
        assert_eq!(snapshot.description, Some("Test snapshot for unit test".to_string()));
        assert_eq!(snapshot.pane_count, 0);
        assert!(snapshot.tabs.is_empty());
    }
}
