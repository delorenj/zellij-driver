use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::env;
use std::process::Stdio;
use tokio::process::Command;

pub struct ZellijDriver;

impl ZellijDriver {
    pub fn new() -> Self {
        Self
    }

    pub fn active_session_name(&self) -> Option<String> {
        env::var("ZELLIJ_SESSION_NAME").ok()
    }

    pub async fn query_tab_names(&self, session: Option<&str>) -> Result<Vec<String>> {
        let output = self.action(session, &["query-tab-names"]).await?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(stdout
            .lines()
            .map(|line| line.trim().to_string())
            .filter(|line| !line.is_empty())
            .collect())
    }

    pub async fn new_tab(&self, session: Option<&str>, name: &str) -> Result<()> {
        self.action(session, &["new-tab", "--name", name]).await?;
        Ok(())
    }

    pub async fn go_to_tab_name(&self, session: Option<&str>, name: &str) -> Result<()> {
        self.action(session, &["go-to-tab-name", name]).await?;
        Ok(())
    }

    pub async fn new_pane(&self, session: Option<&str>) -> Result<()> {
        self.action(session, &["new-pane"]).await?;
        Ok(())
    }

    pub async fn rename_pane(&self, session: Option<&str>, name: &str) -> Result<()> {
        self.action(session, &["rename-pane", name]).await?;
        Ok(())
    }

    pub async fn focus_next_pane(&self, session: Option<&str>) -> Result<()> {
        self.action(session, &["focus-next-pane"]).await?;
        Ok(())
    }

    pub async fn focus_pane_by_index(&self, session: Option<&str>, index: usize) -> Result<()> {
        // Focus panes sequentially to reach target index
        for _ in 0..index {
            self.focus_next_pane(session).await?;
        }
        Ok(())
    }

    pub async fn dump_layout_json(&self, session: Option<&str>) -> Result<Option<Value>> {
        let output = match self.action(session, &["dump-layout", "--json"]).await {
            Ok(output) => output,
            Err(err) => {
                let msg = err.to_string();
                if msg.contains("dump-layout")
                    || msg.contains("unknown")
                    || msg.contains("Unknown")
                    || msg.contains("unrecognized")
                {
                    return Ok(None);
                }
                return Err(err);
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            return Ok(None);
        }

        let value: Value = serde_json::from_str(&stdout).context("failed to parse layout JSON")?;
        Ok(Some(value))
    }

    pub async fn attach_session(&self, session: &str) -> Result<()> {
        let status = Command::new("zellij")
            .arg("attach")
            .arg(session)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .await
            .context("failed to run zellij attach")?;

        if !status.success() {
            return Err(anyhow!("zellij attach failed"));
        }

        Ok(())
    }

    async fn action(&self, session: Option<&str>, args: &[&str]) -> Result<std::process::Output> {
        let mut cmd = Command::new("zellij");
        cmd.arg("action");
        if let Some(session_name) = session {
            cmd.arg("--session").arg(session_name);
        }

        let output = cmd
            .args(args)
            .output()
            .await
            .context("failed to run zellij action")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("zellij action failed: {}", stderr.trim()));
        }

        Ok(output)
    }
}
