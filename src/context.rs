use crate::filter::SecretFilter;
use crate::llm::SessionContext;
use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

const DEFAULT_HISTORY_LINES: usize = 20;
const RECENT_FILE_THRESHOLD_SECS: u64 = 30 * 60; // 30 minutes

/// Collects context from the shell environment for LLM summarization.
pub struct ContextCollector {
    filter: SecretFilter,
    history_lines: usize,
    recent_threshold: Duration,
}

impl ContextCollector {
    /// Create a new context collector with default settings.
    pub fn new() -> Result<Self> {
        Ok(Self {
            filter: SecretFilter::new()?,
            history_lines: DEFAULT_HISTORY_LINES,
            recent_threshold: Duration::from_secs(RECENT_FILE_THRESHOLD_SECS),
        })
    }

    /// Create a context collector with custom settings.
    pub fn with_settings(history_lines: usize, recent_threshold_mins: u64) -> Result<Self> {
        Ok(Self {
            filter: SecretFilter::new()?,
            history_lines,
            recent_threshold: Duration::from_secs(recent_threshold_mins * 60),
        })
    }

    /// Collect context from the current environment.
    pub fn collect(&self, pane_name: &str, cwd: Option<&Path>) -> Result<SessionContext> {
        let working_dir = match cwd {
            Some(p) => p.to_path_buf(),
            None => std::env::current_dir().context("failed to get current directory")?,
        };

        // Collect shell history
        let shell_history = self.collect_shell_history()?;

        // Collect git info if in a git repo
        let (git_branch, git_diff) = self.collect_git_info(&working_dir);

        // Collect recently modified files
        let active_files = self.collect_recent_files(&working_dir)?;

        // Apply secret filtering to all text content
        let (filtered_history, _) = self.filter.filter_lines(&shell_history);
        let filtered_diff = git_diff.map(|d| self.filter.filter(&d).text);

        Ok(SessionContext::new(pane_name)
            .with_cwd(working_dir.display().to_string())
            .with_shell_history(filtered_history)
            .with_active_files(active_files)
            .with_optional_git_branch(git_branch)
            .with_optional_git_diff(filtered_diff))
    }

    /// Collect recent commands from shell history.
    fn collect_shell_history(&self) -> Result<Vec<String>> {
        let histfile = self.find_history_file();

        let Some(path) = histfile else {
            return Ok(Vec::new());
        };

        if !path.exists() {
            return Ok(Vec::new());
        }

        // Read with lossy UTF-8 conversion to handle binary data in history files
        let bytes = fs::read(&path)
            .with_context(|| format!("failed to read history file: {}", path.display()))?;
        let content = String::from_utf8_lossy(&bytes);

        let shell = self.detect_shell();
        let lines = self.parse_history(&content, &shell);

        // Take the last N lines
        Ok(lines
            .into_iter()
            .rev()
            .take(self.history_lines)
            .rev()
            .collect())
    }

    /// Find the appropriate history file based on shell and environment.
    fn find_history_file(&self) -> Option<PathBuf> {
        // First check HISTFILE environment variable
        if let Ok(histfile) = std::env::var("HISTFILE") {
            let path = PathBuf::from(histfile);
            if path.exists() {
                return Some(path);
            }
        }

        // Check home directory for common history files
        let home = std::env::var("HOME").ok()?;
        let home_path = PathBuf::from(home);

        // Try shell-specific history files
        let candidates = [
            home_path.join(".zsh_history"),
            home_path.join(".bash_history"),
            home_path.join(".local/share/fish/fish_history"),
            home_path.join(".history"),
        ];

        candidates.into_iter().find(|p| p.exists())
    }

    /// Detect the current shell type.
    fn detect_shell(&self) -> ShellType {
        // Check SHELL environment variable
        if let Ok(shell) = std::env::var("SHELL") {
            if shell.contains("zsh") {
                return ShellType::Zsh;
            } else if shell.contains("fish") {
                return ShellType::Fish;
            } else if shell.contains("bash") {
                return ShellType::Bash;
            }
        }

        // Check HISTFILE as fallback
        if let Ok(histfile) = std::env::var("HISTFILE") {
            if histfile.contains("zsh") {
                return ShellType::Zsh;
            } else if histfile.contains("fish") {
                return ShellType::Fish;
            }
        }

        ShellType::Bash // Default
    }

    /// Parse history file content based on shell type.
    fn parse_history(&self, content: &str, shell: &ShellType) -> Vec<String> {
        match shell {
            ShellType::Zsh => self.parse_zsh_history(content),
            ShellType::Fish => self.parse_fish_history(content),
            ShellType::Bash => self.parse_bash_history(content),
        }
    }

    /// Parse zsh history format.
    /// Format: `: timestamp:0;command` or just `command`
    fn parse_zsh_history(&self, content: &str) -> Vec<String> {
        content
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.is_empty() {
                    return None;
                }

                // Handle extended history format: : timestamp:0;command
                if line.starts_with(": ") {
                    // Find the semicolon that separates timestamp from command
                    if let Some(idx) = line.find(';') {
                        let cmd = &line[idx + 1..];
                        if !cmd.is_empty() {
                            return Some(cmd.to_string());
                        }
                    }
                    None
                } else {
                    // Simple format - just the command
                    Some(line.to_string())
                }
            })
            .collect()
    }

    /// Parse fish history format (YAML-like).
    /// Format:
    /// - cmd: command
    ///   when: timestamp
    fn parse_fish_history(&self, content: &str) -> Vec<String> {
        content
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                if line.starts_with("- cmd:") {
                    Some(line.trim_start_matches("- cmd:").trim().to_string())
                } else {
                    None
                }
            })
            .collect()
    }

    /// Parse bash history format (simple line-per-command).
    fn parse_bash_history(&self, content: &str) -> Vec<String> {
        content
            .lines()
            .filter_map(|line| {
                let line = line.trim();
                // Skip timestamp lines (start with #)
                if line.is_empty() || line.starts_with('#') {
                    None
                } else {
                    Some(line.to_string())
                }
            })
            .collect()
    }

    /// Collect git branch and diff information.
    fn collect_git_info(&self, cwd: &Path) -> (Option<String>, Option<String>) {
        // Check if we're in a git repo
        let is_git = Command::new("git")
            .args(["rev-parse", "--is-inside-work-tree"])
            .current_dir(cwd)
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !is_git {
            return (None, None);
        }

        // Get current branch
        let branch = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(cwd)
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8(o.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                } else {
                    None
                }
            });

        // Get diff stat
        let diff = Command::new("git")
            .args(["diff", "--stat"])
            .current_dir(cwd)
            .output()
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    String::from_utf8(o.stdout)
                        .ok()
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                } else {
                    None
                }
            });

        (branch, diff)
    }

    /// Collect files modified within the recent threshold.
    fn collect_recent_files(&self, cwd: &Path) -> Result<Vec<String>> {
        let now = SystemTime::now();
        let mut recent = Vec::new();

        self.walk_dir_recent(cwd, cwd, &now, &mut recent)?;

        // Sort by path for consistency
        recent.sort();

        // Limit to 20 files max
        recent.truncate(20);

        Ok(recent)
    }

    /// Recursively walk directory looking for recently modified files.
    fn walk_dir_recent(
        &self,
        base: &Path,
        dir: &Path,
        now: &SystemTime,
        results: &mut Vec<String>,
    ) -> Result<()> {
        let entries = match fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return Ok(()), // Skip unreadable directories
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            let file_name = entry.file_name().to_string_lossy().to_string();

            // Skip hidden files and common non-source directories
            if file_name.starts_with('.')
                || file_name == "node_modules"
                || file_name == "target"
                || file_name == "dist"
                || file_name == "build"
                || file_name == "__pycache__"
                || file_name == ".git"
            {
                continue;
            }

            if path.is_dir() {
                self.walk_dir_recent(base, &path, now, results)?;
            } else if path.is_file() {
                if let Ok(metadata) = path.metadata() {
                    if let Ok(modified) = metadata.modified() {
                        if let Ok(elapsed) = now.duration_since(modified) {
                            if elapsed < self.recent_threshold {
                                // Make path relative to base
                                if let Ok(relative) = path.strip_prefix(base) {
                                    results.push(relative.display().to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for ContextCollector {
    fn default() -> Self {
        Self::new().expect("default context collector should be creatable")
    }
}

#[derive(Debug, Clone, Copy)]
enum ShellType {
    Bash,
    Zsh,
    Fish,
}

// Extension trait for SessionContext to support optional fields
trait SessionContextExt {
    fn with_optional_git_branch(self, branch: Option<String>) -> Self;
    fn with_optional_git_diff(self, diff: Option<String>) -> Self;
}

impl SessionContextExt for SessionContext {
    fn with_optional_git_branch(self, branch: Option<String>) -> Self {
        match branch {
            Some(b) => self.with_git_branch(b),
            None => self,
        }
    }

    fn with_optional_git_diff(self, diff: Option<String>) -> Self {
        match diff {
            Some(d) => self.with_git_diff(d),
            None => self,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bash_history() {
        let collector = ContextCollector::new().unwrap();

        let content = "git status\ncargo build\n#12345678\nnpm install\n";
        let parsed = collector.parse_bash_history(content);

        assert_eq!(parsed, vec!["git status", "cargo build", "npm install"]);
    }

    #[test]
    fn test_parse_zsh_history() {
        let collector = ContextCollector::new().unwrap();

        // Extended format
        let content = ": 1704067200:0;git status\n: 1704067201:0;cargo build\n";
        let parsed = collector.parse_zsh_history(content);

        assert_eq!(parsed, vec!["git status", "cargo build"]);
    }

    #[test]
    fn test_parse_zsh_history_simple() {
        let collector = ContextCollector::new().unwrap();

        // Simple format (no timestamps)
        let content = "git status\ncargo build\n";
        let parsed = collector.parse_zsh_history(content);

        assert_eq!(parsed, vec!["git status", "cargo build"]);
    }

    #[test]
    fn test_parse_fish_history() {
        let collector = ContextCollector::new().unwrap();

        let content = "- cmd: git status\n  when: 1704067200\n- cmd: cargo build\n  when: 1704067201\n";
        let parsed = collector.parse_fish_history(content);

        assert_eq!(parsed, vec!["git status", "cargo build"]);
    }

    #[test]
    fn test_detect_shell_from_env() {
        // This test just verifies the detect_shell method doesn't panic
        let collector = ContextCollector::new().unwrap();
        let _shell = collector.detect_shell();
    }

    #[test]
    fn test_collect_git_info_in_git_repo() {
        let collector = ContextCollector::new().unwrap();
        let cwd = std::env::current_dir().unwrap();

        let (branch, _diff) = collector.collect_git_info(&cwd);

        // We should be in a git repo for this project
        assert!(branch.is_some(), "Expected to find a git branch");
    }

    #[test]
    fn test_collect_git_info_outside_repo() {
        let collector = ContextCollector::new().unwrap();

        // /tmp is typically not a git repo
        let (branch, diff) = collector.collect_git_info(Path::new("/tmp"));

        assert!(branch.is_none());
        assert!(diff.is_none());
    }

    #[test]
    fn test_context_collector_default() {
        let collector = ContextCollector::default();
        assert_eq!(collector.history_lines, DEFAULT_HISTORY_LINES);
    }

    #[test]
    fn test_collect_basic() {
        let collector = ContextCollector::new().unwrap();
        let cwd = std::env::current_dir().unwrap();

        let context = collector.collect("test-pane", Some(&cwd)).unwrap();

        assert_eq!(context.pane_name, "test-pane");
        assert!(!context.cwd.is_empty());
    }
}
