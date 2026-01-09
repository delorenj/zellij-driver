use crate::types::{RestoreReport, RestoreWarning, SessionSnapshot, TabSnapshot};
use crate::zellij::ZellijDriver;
use anyhow::{anyhow, Context, Result};
use chrono::Utc;

/// Session restoration module.
///
/// Handles recreating Zellij sessions from snapshots, including tabs, panes,
/// working directories, and layout configuration.
pub struct SessionRestore {
    zellij: ZellijDriver,
}

impl SessionRestore {
    pub fn new(zellij: ZellijDriver) -> Self {
        Self { zellij }
    }

    /// Restore a session from a snapshot.
    ///
    /// # Arguments
    /// * `snapshot` - The snapshot to restore
    /// * `dry_run` - If true, only plan restoration without making changes
    ///
    /// # Returns
    /// A RestoreReport with status, counters, and warnings
    pub async fn restore_session(
        &self,
        snapshot: &SessionSnapshot,
        dry_run: bool,
    ) -> Result<RestoreReport> {
        // Get current session
        let current_session = self
            .zellij
            .active_session_name()
            .ok_or_else(|| anyhow!("not inside a zellij session; restore requires active session"))?;

        // Initialize report
        let mut report = RestoreReport::new(snapshot.name.clone(), current_session);
        let start_time = Utc::now();

        // Get existing tabs to avoid duplicates
        let existing_tabs = if !dry_run {
            self.zellij.query_tab_names(None).await?
        } else {
            vec![]
        };

        // Restore each tab
        for tab in &snapshot.tabs {
            match self.restore_tab(tab, &existing_tabs, dry_run, &mut report).await {
                Ok(_) => {
                    report.tabs_restored += 1;
                }
                Err(e) => {
                    report.tabs_failed += 1;
                    let warning = RestoreWarning::error(format!("Failed to restore tab: {}", e))
                        .for_component(format!("tab '{}'", tab.name));
                    report.add_warning(warning);
                }
            }
        }

        // Calculate duration
        let duration = Utc::now().signed_duration_since(start_time);
        report.duration_ms = duration.num_milliseconds() as u64;

        Ok(report)
    }

    /// Restore a single tab from snapshot.
    async fn restore_tab(
        &self,
        tab: &TabSnapshot,
        existing_tabs: &[String],
        dry_run: bool,
        report: &mut RestoreReport,
    ) -> Result<()> {
        // Check if tab already exists
        let tab_exists = existing_tabs.iter().any(|t| t == &tab.name);

        if dry_run {
            if tab_exists {
                let warning = RestoreWarning::info(format!("Tab '{}' already exists, would skip creation", tab.name))
                    .for_component(format!("tab '{}'", tab.name));
                report.add_warning(warning);
            } else {
                // Just log what we would do
                println!("  [DRY RUN] Would create tab: {}", tab.name);
            }

            // Log panes that would be created
            for pane in &tab.panes {
                println!("    [DRY RUN] Would create pane: {} at position {}", pane.name, pane.position);
                if let Some(cwd) = &pane.cwd {
                    println!("      CWD: {}", cwd);
                }
            }

            return Ok(());
        }

        // Create or switch to tab
        if tab_exists {
            self.zellij.go_to_tab_name(None, &tab.name).await
                .context("failed to switch to existing tab")?;

            let warning = RestoreWarning::info(format!("Tab '{}' already exists, switching to it", tab.name))
                .for_component(format!("tab '{}'", tab.name));
            report.add_warning(warning);
        } else {
            self.zellij.new_tab(None, &tab.name).await
                .context("failed to create tab")?;
        }

        // Restore panes in this tab
        for (idx, pane) in tab.panes.iter().enumerate() {
            match self.restore_pane(pane, idx, &tab.name, report).await {
                Ok(_) => {
                    report.panes_restored += 1;
                }
                Err(e) => {
                    report.panes_failed += 1;
                    let warning = RestoreWarning::warning(format!("Failed to restore pane: {}", e))
                        .for_component(format!("tab '{}', pane '{}'", tab.name, pane.name));
                    report.add_warning(warning);
                }
            }
        }

        Ok(())
    }

    /// Restore a single pane.
    async fn restore_pane(
        &self,
        pane: &crate::types::PaneSnapshot,
        index: usize,
        tab_name: &str,
        report: &mut RestoreReport,
    ) -> Result<()> {
        // Skip first pane (already exists when tab is created)
        if index == 0 {
            // Just rename it
            self.zellij.rename_pane(None, &pane.name).await
                .context("failed to rename first pane")?;

            if pane.name == "unnamed" {
                let warning = RestoreWarning::info("First pane has no name")
                    .for_component(format!("tab '{}'", tab_name));
                report.add_warning(warning);
            }

            return Ok(());
        }

        // Create new pane (default to vertical split)
        let direction = if index % 2 == 0 { "down" } else { "right" };

        if let Some(cwd) = &pane.cwd {
            self.zellij.new_pane_with_cwd(None, cwd, direction).await
                .context("failed to create pane with CWD")?;
        } else {
            if direction == "down" {
                self.zellij.new_pane_horizontal(None).await
                    .context("failed to create horizontal pane")?;
            } else {
                self.zellij.new_pane_vertical(None).await
                    .context("failed to create vertical pane")?;
            }
        }

        // Rename pane
        self.zellij.rename_pane(None, &pane.name).await
            .context("failed to rename pane")?;

        // Warn if pane has no CWD
        if pane.cwd.is_none() {
            let warning = RestoreWarning::info("Pane has no saved working directory")
                .for_component(format!("tab '{}', pane '{}'", tab_name, pane.name));
            report.add_warning(warning);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PaneSnapshot;
    use std::collections::HashMap;

    #[test]
    fn test_restore_report_initialization() {
        let report = RestoreReport::new("test-snapshot", "test-session");
        assert_eq!(report.snapshot_name, "test-snapshot");
        assert_eq!(report.session, "test-session");
        assert_eq!(report.tabs_restored, 0);
        assert_eq!(report.panes_restored, 0);
    }

    #[test]
    fn test_pane_snapshot_structure() {
        let pane = PaneSnapshot {
            name: "test-pane".to_string(),
            position: 0,
            cwd: Some("/home/user".to_string()),
            command: None,
            pane_id: Some("42".to_string()),
            focused: true,
            meta: HashMap::new(),
        };

        assert_eq!(pane.name, "test-pane");
        assert_eq!(pane.position, 0);
        assert!(pane.focused);
    }
}
