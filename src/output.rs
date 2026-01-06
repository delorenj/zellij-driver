use crate::types::{IntentEntry, IntentSource, IntentType};
use chrono::{DateTime, Local, Utc};
use chrono_humanize::HumanTime;
use colored::Colorize;
use std::io::IsTerminal;

pub struct OutputFormatter {
    use_color: bool,
    terminal_width: Option<usize>,
}

impl OutputFormatter {
    pub fn new() -> Self {
        let use_color = std::env::var("NO_COLOR").is_err() && std::io::stdout().is_terminal();
        let terminal_width = terminal_size::terminal_size().map(|(w, _)| w.0 as usize);

        Self {
            use_color,
            terminal_width,
        }
    }

    pub fn format_history(&self, entries: &[IntentEntry], pane_name: &str) -> String {
        if entries.is_empty() {
            return format!("No history for pane '{}'", pane_name);
        }

        let mut output = Vec::new();

        for entry in entries {
            output.push(self.format_entry(entry));
        }

        output.join("\n\n")
    }

    fn format_entry(&self, entry: &IntentEntry) -> String {
        let mut lines = Vec::new();

        // Header line: type badge + source badge (if agent) + relative time
        let type_badge = self.format_type_badge(entry.entry_type);
        let source_badge = self.format_source_badge(entry.source);
        let time_str = self.format_relative_time(entry.timestamp);

        if source_badge.is_empty() {
            lines.push(format!("{} {}", type_badge, time_str));
        } else {
            lines.push(format!("{} {} {}", type_badge, source_badge, time_str));
        }

        // Summary line with wrapping
        let summary = self.wrap_text(&entry.summary, 2);
        lines.push(summary);

        // Artifacts if present
        if !entry.artifacts.is_empty() {
            for artifact in &entry.artifacts {
                let artifact_line = if self.use_color {
                    format!("  {} {}", "→".dimmed(), artifact.dimmed())
                } else {
                    format!("  -> {}", artifact)
                };
                lines.push(artifact_line);
            }
        }

        lines.join("\n")
    }

    fn format_type_badge(&self, entry_type: IntentType) -> String {
        let (icon, label) = match entry_type {
            IntentType::Milestone => ("★", "MILESTONE"),
            IntentType::Checkpoint => ("●", "CHECKPOINT"),
            IntentType::Exploration => ("◈", "EXPLORATION"),
        };

        if self.use_color {
            let badge = format!("[{} {}]", icon, label);
            match entry_type {
                IntentType::Milestone => badge.yellow().bold().to_string(),
                IntentType::Checkpoint => badge.green().to_string(),
                IntentType::Exploration => badge.cyan().to_string(),
            }
        } else {
            format!("[{} {}]", icon, label)
        }
    }

    fn format_source_badge(&self, source: IntentSource) -> String {
        match source {
            IntentSource::Manual => String::new(), // Default, no badge
            IntentSource::Automated => {
                if self.use_color {
                    "[⚡ AUTO]".blue().to_string()
                } else {
                    "[⚡ AUTO]".to_string()
                }
            }
            IntentSource::Agent => {
                if self.use_color {
                    "[🤖 AGENT]".magenta().bold().to_string()
                } else {
                    "[🤖 AGENT]".to_string()
                }
            }
        }
    }

    fn format_relative_time(&self, timestamp: DateTime<Utc>) -> String {
        let local_time: DateTime<Local> = timestamp.into();
        let human_time = HumanTime::from(timestamp);

        let relative = human_time.to_string();
        let absolute = local_time.format("%Y-%m-%d %H:%M").to_string();

        if self.use_color {
            format!("{} ({})", relative.dimmed(), absolute.dimmed())
        } else {
            format!("{} ({})", relative, absolute)
        }
    }

    /// Format history as LLM-optimized context for prompt injection.
    /// Produces a compact narrative optimized for ~1000 tokens.
    pub fn format_context(&self, entries: &[IntentEntry], pane_name: &str) -> String {
        let mut output = Vec::new();

        // Header with session context
        output.push(format!("## Session Context: {}", pane_name));
        output.push(String::new());

        if entries.is_empty() {
            output.push("This is a new session with no prior history.".to_string());
            output.push(String::new());
            output.push("### Recommended First Steps".to_string());
            output.push("1. Review the current codebase state".to_string());
            output.push("2. Identify the main objective for this session".to_string());
            output.push("3. Log your initial intent with `zdrive pane log`".to_string());
            return output.join("\n");
        }

        // Calculate session stats
        let total_entries = entries.len();
        let milestone_count = entries.iter().filter(|e| e.entry_type == IntentType::Milestone).count();
        let agent_count = entries.iter().filter(|e| e.source == IntentSource::Agent).count();
        let human_count = entries.iter().filter(|e| e.source == IntentSource::Manual).count();

        // Session overview
        output.push("### Session Overview".to_string());
        output.push(format!("- Total entries: {} ({} milestones)", total_entries, milestone_count));
        if agent_count > 0 {
            output.push(format!("- Agent contributions: {} entries", agent_count));
        }
        if human_count > 0 {
            output.push(format!("- Human entries: {}", human_count));
        }

        // Time span
        if let (Some(newest), Some(oldest)) = (entries.first(), entries.last()) {
            let duration = newest.timestamp - oldest.timestamp;
            let hours = duration.num_hours();
            let mins = duration.num_minutes() % 60;
            if hours > 0 {
                output.push(format!("- Session duration: {}h {}m", hours, mins));
            } else if mins > 0 {
                output.push(format!("- Session duration: {}m", mins));
            }
        }
        output.push(String::new());

        // Recent activity (limit to last 5 entries for token efficiency)
        output.push("### Recent Activity".to_string());
        let recent_entries: Vec<_> = entries.iter().take(5).collect();
        for entry in &recent_entries {
            let type_marker = match entry.entry_type {
                IntentType::Milestone => "🌟 MILESTONE",
                IntentType::Checkpoint => "●",
                IntentType::Exploration => "🔍",
            };
            let source_marker = match entry.source {
                IntentSource::Agent => " [agent]",
                IntentSource::Automated => " [auto]",
                IntentSource::Manual => "",
            };
            let time = entry.timestamp.format("%H:%M").to_string();
            output.push(format!("- {} ({}{}) {}", type_marker, time, source_marker, entry.summary));

            // Include artifacts for milestones (they're important)
            if entry.entry_type == IntentType::Milestone && !entry.artifacts.is_empty() {
                for artifact in &entry.artifacts {
                    output.push(format!("  - `{}`", artifact));
                }
            }
        }
        output.push(String::new());

        // Last checkpoint (most recent entry)
        if let Some(last) = entries.first() {
            output.push("### Current State".to_string());
            output.push(format!("Last checkpoint: **{}**", last.summary));
            if !last.artifacts.is_empty() {
                output.push(format!("Key files: {}", last.artifacts.join(", ")));
            }
            output.push(String::new());
        }

        // Identify milestones for context
        let milestones: Vec<_> = entries.iter()
            .filter(|e| e.entry_type == IntentType::Milestone)
            .take(3)
            .collect();

        if !milestones.is_empty() {
            output.push("### Key Milestones".to_string());
            for m in milestones {
                output.push(format!("- {} ({})", m.summary, m.timestamp.format("%Y-%m-%d")));
            }
            output.push(String::new());
        }

        // Suggested next steps based on history
        output.push("### Suggested Next Steps".to_string());
        if let Some(last) = entries.first() {
            match last.entry_type {
                IntentType::Exploration => {
                    output.push("1. Review findings from the exploration".to_string());
                    output.push("2. Decide on implementation approach".to_string());
                    output.push("3. Log a milestone when committing to a direction".to_string());
                }
                IntentType::Milestone => {
                    output.push("1. Verify the milestone is stable".to_string());
                    output.push("2. Identify the next feature or fix to tackle".to_string());
                    output.push("3. Log a checkpoint to track progress".to_string());
                }
                IntentType::Checkpoint => {
                    output.push("1. Continue from the last checkpoint".to_string());
                    output.push("2. Log progress as you work".to_string());
                    output.push("3. Mark significant achievements as milestones".to_string());
                }
            }
        }

        output.join("\n")
    }

    pub fn format_markdown(&self, entries: &[IntentEntry], pane_name: &str) -> String {
        let mut output = Vec::new();

        // YAML frontmatter
        output.push("---".to_string());
        output.push(format!("pane: {}", pane_name));
        output.push(format!("entries: {}", entries.len()));
        if let Some(first) = entries.first() {
            output.push(format!("latest: {}", first.timestamp.format("%Y-%m-%d")));
        }
        if let Some(last) = entries.last() {
            output.push(format!("earliest: {}", last.timestamp.format("%Y-%m-%d")));
        }
        output.push(format!("exported: {}", chrono::Local::now().format("%Y-%m-%dT%H:%M:%S")));
        output.push("---".to_string());
        output.push(String::new());

        // Title
        output.push(format!("# Session: {}", pane_name));
        output.push(String::new());

        if entries.is_empty() {
            output.push("*No entries recorded.*".to_string());
            return output.join("\n");
        }

        // Group entries by date
        let mut current_date = String::new();

        for entry in entries {
            let entry_date = entry.timestamp.format("%Y-%m-%d").to_string();

            if entry_date != current_date {
                if !current_date.is_empty() {
                    output.push(String::new());
                }
                output.push(format!("## {}", entry_date));
                output.push(String::new());
                current_date = entry_date;
            }

            // Entry line with type emoji, source tag, and time
            let emoji = match entry.entry_type {
                IntentType::Milestone => "🌟",
                IntentType::Checkpoint => "📍",
                IntentType::Exploration => "🔍",
            };

            let source_tag = match entry.source {
                IntentSource::Manual => "",
                IntentSource::Automated => " ⚡",
                IntentSource::Agent => " 🤖",
            };

            let time = entry.timestamp.format("%H:%M").to_string();
            output.push(format!("- {}{} **{}** {}", emoji, source_tag, time, entry.summary));

            // Artifacts as sub-bullets with file links
            for artifact in &entry.artifacts {
                // Create Obsidian-compatible file link if it looks like a path
                if artifact.contains('/') || artifact.contains('.') {
                    output.push(format!("  - `{}`", artifact));
                } else {
                    output.push(format!("  - {}", artifact));
                }
            }
        }

        output.join("\n")
    }

    fn wrap_text(&self, text: &str, indent: usize) -> String {
        let width = self.terminal_width.unwrap_or(80);
        let available = width.saturating_sub(indent);

        if text.len() <= available {
            return format!("{:indent$}{}", "", text, indent = indent);
        }

        let mut lines = Vec::new();
        let mut current_line = String::new();
        let indent_str = " ".repeat(indent);

        for word in text.split_whitespace() {
            if current_line.is_empty() {
                current_line = word.to_string();
            } else if current_line.len() + 1 + word.len() <= available {
                current_line.push(' ');
                current_line.push_str(word);
            } else {
                lines.push(format!("{}{}", indent_str, current_line));
                current_line = word.to_string();
            }
        }

        if !current_line.is_empty() {
            lines.push(format!("{}{}", indent_str, current_line));
        }

        lines.join("\n")
    }
}

impl Default for OutputFormatter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::IntentEntry;

    #[test]
    fn test_format_empty_history() {
        let formatter = OutputFormatter {
            use_color: false,
            terminal_width: Some(80),
        };
        let result = formatter.format_history(&[], "test-pane");
        assert_eq!(result, "No history for pane 'test-pane'");
    }

    #[test]
    fn test_format_type_badge_no_color() {
        let formatter = OutputFormatter {
            use_color: false,
            terminal_width: Some(80),
        };

        assert_eq!(
            formatter.format_type_badge(IntentType::Milestone),
            "[★ MILESTONE]"
        );
        assert_eq!(
            formatter.format_type_badge(IntentType::Checkpoint),
            "[● CHECKPOINT]"
        );
        assert_eq!(
            formatter.format_type_badge(IntentType::Exploration),
            "[◈ EXPLORATION]"
        );
    }

    #[test]
    fn test_wrap_text() {
        let formatter = OutputFormatter {
            use_color: false,
            terminal_width: Some(40),
        };

        let short = "Short text";
        assert_eq!(formatter.wrap_text(short, 2), "  Short text");

        let long = "This is a longer text that should wrap across multiple lines";
        let wrapped = formatter.wrap_text(long, 2);
        for line in wrapped.lines() {
            assert!(line.len() <= 40);
        }
    }

    #[test]
    fn test_format_entry_with_artifacts() {
        let formatter = OutputFormatter {
            use_color: false,
            terminal_width: Some(80),
        };

        let entry = IntentEntry::new("Implemented feature X")
            .with_type(IntentType::Milestone)
            .with_artifacts(vec!["src/feature.rs".to_string()]);

        let formatted = formatter.format_entry(&entry);
        assert!(formatted.contains("[★ MILESTONE]"));
        assert!(formatted.contains("Implemented feature X"));
        assert!(formatted.contains("src/feature.rs"));
    }
}
