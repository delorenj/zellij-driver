use anyhow::{anyhow, Context, Result};
use semver::{Version, VersionReq};
use serde_json::Value;
use std::env;
use std::process::Stdio;
use std::sync::OnceLock;
use tokio::process::Command;

const MIN_ZELLIJ_VERSION: &str = ">=0.39.0";

static VERSION_CHECK: OnceLock<Result<Version, String>> = OnceLock::new();

pub struct ZellijDriver;

impl ZellijDriver {
    pub fn new() -> Self {
        Self
    }

    /// Check Zellij version meets minimum requirements.
    /// This is cached after the first successful check.
    pub async fn check_version(&self) -> Result<Version> {
        // Return cached result if available
        if let Some(result) = VERSION_CHECK.get() {
            return result
                .clone()
                .map_err(|e| anyhow!("{}", e));
        }

        let result = self.get_zellij_version().await;

        match &result {
            Ok(version) => {
                let req = VersionReq::parse(MIN_ZELLIJ_VERSION)
                    .expect("invalid version requirement");

                if !req.matches(version) {
                    let err_msg = format!(
                        "Zellij version {} is too old. Perth requires Zellij {} or later.\n\
                         \n\
                         To upgrade Zellij:\n\
                         • Cargo: cargo install zellij --locked\n\
                         • Homebrew: brew upgrade zellij\n\
                         • Linux: https://zellij.dev/documentation/installation",
                        version, MIN_ZELLIJ_VERSION.trim_start_matches(">=")
                    );
                    let _ = VERSION_CHECK.set(Err(err_msg.clone()));
                    return Err(anyhow!("{}", err_msg));
                }

                let _ = VERSION_CHECK.set(Ok(version.clone()));
                Ok(version.clone())
            }
            Err(e) => {
                let err_msg = e.to_string();
                let _ = VERSION_CHECK.set(Err(err_msg.clone()));
                Err(anyhow!("{}", err_msg))
            }
        }
    }

    async fn get_zellij_version(&self) -> Result<Version> {
        let output = Command::new("zellij")
            .arg("--version")
            .output()
            .await
            .context("failed to run 'zellij --version'. Is Zellij installed?")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("zellij --version failed: {}", stderr.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        // Output format: "zellij 0.39.0" or similar
        let version_str = stdout
            .trim()
            .strip_prefix("zellij ")
            .unwrap_or(stdout.trim());

        Version::parse(version_str)
            .with_context(|| format!("failed to parse Zellij version: {}", version_str))
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

    /// Create a new pane with vertical split (side by side)
    pub async fn new_pane_vertical(&self, session: Option<&str>) -> Result<()> {
        self.action(session, &["new-pane", "--direction", "right"]).await?;
        Ok(())
    }

    /// Create a new pane with horizontal split (stacked)
    pub async fn new_pane_horizontal(&self, session: Option<&str>) -> Result<()> {
        self.action(session, &["new-pane", "--direction", "down"]).await?;
        Ok(())
    }

    /// Create a new pane with specified working directory
    pub async fn new_pane_with_cwd(&self, session: Option<&str>, cwd: &str, direction: &str) -> Result<()> {
        self.action(session, &["new-pane", "--direction", direction, "--cwd", cwd]).await?;
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
