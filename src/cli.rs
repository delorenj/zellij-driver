use crate::types::{IntentSource, IntentType};
use clap::{Args, Parser, Subcommand, ValueEnum};
use std::collections::HashMap;

/// Split direction for pane creation
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum SplitDirection {
    /// Vertical split (panes side by side)
    #[default]
    Vertical,
    /// Horizontal split (panes stacked)
    Horizontal,
}

/// Output format for commands
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text (default)
    #[default]
    Text,
    /// Pretty-printed JSON
    Json,
    /// Compact single-line JSON
    JsonCompact,
    /// Markdown with YAML frontmatter (Obsidian-compatible)
    Markdown,
    /// LLM-optimized context for prompt injection (~1000 tokens)
    Context,
}

#[derive(Parser)]
#[command(version, about = "Redis-backed Zellij pane manager")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    Pane(PaneArgs),
    Tab(TabArgs),
    Reconcile,
    /// List all known panes organized by session and tab
    List,
    /// Migrate data from v1.0 (znav:*) to v2.0 (perth:*) keyspace
    Migrate(MigrateArgs),
    /// View or modify configuration settings
    Config(ConfigArgs),
}

#[derive(Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub action: ConfigAction,
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Display current configuration settings
    ///
    /// Shows all configuration values including defaults,
    /// custom settings, and the location of the config file.
    #[command(
        after_help = "EXAMPLES:
    # View all configuration settings
    zdrive config show

    # View config and pipe to grep
    zdrive config show | grep redis

CONFIG FILE LOCATION:
    $XDG_CONFIG_HOME/zellij-driver/config.toml
    or ~/.config/zellij-driver/config.toml"
    )]
    Show,

    /// Set a configuration value
    ///
    /// Updates a configuration setting and saves it to the config file.
    /// Creates the config file if it doesn't exist.
    #[command(
        after_help = "EXAMPLES:
    # Set Redis URL
    zdrive config set redis_url redis://localhost:6379/

    # Set Redis with authentication
    zdrive config set redis_url redis://:password@localhost:6379/0

AVAILABLE SETTINGS:
    redis_url    Redis connection URL (default: redis://127.0.0.1:6379/)"
    )]
    Set {
        /// Configuration key to set
        #[arg(help = "The configuration key (e.g., 'redis_url')")]
        key: String,

        /// New value for the configuration key
        #[arg(help = "The new value for the key")]
        value: String,
    },

    /// Manage consent for sending data to LLM providers
    ///
    /// The snapshot command sends shell history, git diff, and file information
    /// to an LLM provider for summarization. This requires explicit user consent.
    #[command(
        after_help = "EXAMPLES:
    # Grant consent for LLM data sharing
    zdrive config consent --grant

    # Revoke previously granted consent
    zdrive config consent --revoke

    # Check current consent status
    zdrive config show | grep consent

WHAT DATA IS SHARED:
    When using the snapshot command with an LLM provider, the following
    data may be sent to the provider's servers:

    - Shell command history (last ~50 commands)
    - Git diff output showing recent changes
    - Current working directory path
    - Names of recently modified files

    This data is used to generate an AI-powered summary of your work.
    No data is sent without your explicit consent.

PRIVACY NOTES:
    - Secrets (API keys, passwords, tokens) are automatically filtered
    - Data is sent only when you run the 'snapshot' command
    - You can revoke consent at any time
    - The 'none' provider never sends any data"
    )]
    Consent {
        /// Grant consent for LLM data sharing
        #[arg(long, conflicts_with = "revoke")]
        grant: bool,

        /// Revoke consent for LLM data sharing
        #[arg(long, conflicts_with = "grant")]
        revoke: bool,
    },
}

#[derive(Args)]
pub struct MigrateArgs {
    /// Show what would be migrated without making changes
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Args)]
pub struct PaneArgs {
    #[command(subcommand)]
    pub action: Option<PaneAction>,
    pub name: Option<String>,
    #[arg(long)]
    pub tab: Option<String>,
    #[arg(long)]
    pub session: Option<String>,
    #[arg(long = "meta", value_parser = parse_key_val)]
    pub meta: Vec<(String, String)>,
}

#[derive(Subcommand)]
pub enum PaneAction {
    /// Get info about a pane
    Info { name: String },

    /// Spawn multiple named panes in a single command
    ///
    /// Creates multiple panes in a tab for parallel work. Each pane is named
    /// according to the list and registered in Redis with position metadata.
    #[command(
        after_help = "EXAMPLES:
    # Create 3 panes in a tab
    znav pane batch --tab \"myapp(fixes)\" --panes fix-auth,fix-errors,fix-docs

    # Create with working directories
    znav pane batch --tab \"myapp(fixes)\" \\
        --panes fix-auth,fix-errors,fix-docs \\
        --cwd ../fix-auth,../fix-errors,../fix-docs

    # Use horizontal layout (stacked)
    znav pane batch --tab \"myapp(fixes)\" --panes a,b,c --layout horizontal

LAYOUT OPTIONS:
    vertical     Panes arranged side by side (default)
    horizontal   Panes stacked top to bottom

NOTES:
    - Creates panes sequentially in the specified tab
    - If --cwd has fewer entries than --panes, remaining panes use current dir
    - All panes are registered in Redis for tracking

RELATED COMMANDS:
    znav tab create         Create a tab first
    znav pane log           Log intent for each pane
    znav list               View all panes"
    )]
    Batch {
        /// Tab to create panes in (required)
        #[arg(short = 't', long, help = "Tab name to create panes in")]
        tab: String,

        /// Comma-separated list of pane names
        #[arg(short = 'p', long, value_delimiter = ',',
              help = "Pane names (e.g., 'fix-auth,fix-errors,fix-docs')")]
        panes: Vec<String>,

        /// Comma-separated list of working directories (optional)
        #[arg(short = 'c', long, value_delimiter = ',',
              help = "Working directories for each pane (e.g., '../dir1,../dir2')")]
        cwd: Vec<String>,

        /// Split layout direction
        #[arg(short = 'l', long, default_value = "vertical", value_enum,
              help = "Pane layout: vertical (side by side) or horizontal (stacked)")]
        layout: SplitDirection,
    },

    /// Auto-generate an intent summary from recent work using LLM
    ///
    /// Collects context (shell history, git diff, modified files) and uses
    /// an LLM provider to generate a summary of your recent work.
    #[command(
        after_help = "EXAMPLES:
    # Generate a snapshot for a pane
    zdrive pane snapshot my-feature

    # Generate snapshot and view the result
    zdrive pane snapshot my-feature && zdrive pane history my-feature --last 1

CONFIGURATION:
    Requires an LLM provider to be configured. Set up in config:
    zdrive config set llm.provider anthropic
    zdrive config set llm.anthropic_api_key YOUR_API_KEY

    Or via environment variable:
    export ANTHROPIC_API_KEY=YOUR_API_KEY

RELATED COMMANDS:
    zdrive pane log <PANE> <SUMMARY>  Manual entry logging
    zdrive pane history <PANE>        View logged entries"
    )]
    Snapshot {
        /// Pane name to generate snapshot for
        #[arg(help = "Name of the pane to snapshot")]
        name: String,
    },

    /// Log an intent entry to track your work on a pane
    ///
    /// Record what you're working on, accomplishments, and discoveries.
    /// Each entry is timestamped and stored in Redis for later review.
    #[command(
        after_help = "EXAMPLES:
    # Log a simple checkpoint
    zdrive pane log my-feature \"Fixed authentication bug\"

    # Log a milestone with artifacts
    zdrive pane log api-refactor \"Completed REST API redesign\" \\
        --type milestone --artifacts src/api.rs docs/api.md

    # Log an exploration session
    zdrive pane log research \"Investigated caching strategies\" --type exploration

    # Log from an AI agent (for agent integration)
    zdrive pane log my-feature \"Completed task analysis\" --source agent

RELATED COMMANDS:
    zdrive pane history <PANE>  View logged entries
    zdrive pane info <PANE>     Check pane status"
    )]
    Log {
        /// Pane name to log the entry for
        #[arg(help = "Name of the pane to log this entry for")]
        name: String,

        /// Brief description of what you accomplished or worked on
        #[arg(help = "Summary of your work (e.g., 'Fixed login timeout issue')")]
        summary: String,

        /// Categorize this entry by type
        ///
        /// - checkpoint: Regular progress marker (default)
        /// - milestone: Major accomplishment worth highlighting
        /// - exploration: Research or investigation work
        #[arg(short = 't', long, default_value = "checkpoint", value_enum,
              help = "Entry type: checkpoint (default), milestone, or exploration")]
        entry_type: IntentType,

        /// Source of this log entry
        ///
        /// - manual: Human-created entry (default)
        /// - agent: Created by an AI agent during assisted workflow
        #[arg(short = 's', long, default_value = "manual", value_enum,
              help = "Entry source: manual (default) or agent")]
        source: IntentSource,

        /// Files or paths related to this work
        ///
        /// Useful for tracking which files were modified or created.
        /// Paths are resolved to absolute paths when possible.
        #[arg(short = 'a', long = "artifacts", num_args = 1..,
              help = "Files or artifacts associated with this work")]
        artifacts: Vec<String>,
    },

    /// View the intent history for a pane
    ///
    /// Shows logged entries with timestamps, types, and artifacts.
    /// Supports multiple output formats for different use cases.
    #[command(
        after_help = "EXAMPLES:
    # View all history in human-readable format
    zdrive pane history my-feature

    # View last 5 entries
    zdrive pane history my-feature --last 5

    # Export to JSON for tooling integration
    zdrive pane history my-feature --format json

    # Compact JSON for piping to other commands
    zdrive pane history my-feature --format json-compact | jq '.entries[0]'

    # Export to Markdown for Obsidian
    zdrive pane history my-feature --format markdown > ~/notes/sessions/my-feature.md

    # Get LLM-optimized context for agent integration
    zdrive pane history my-feature --format context

OUTPUT FORMATS:
    text         Human-readable with colors and relative timestamps
    json         Pretty-printed JSON with schema version
    json-compact Single-line JSON for scripting
    markdown     Markdown with YAML frontmatter (Obsidian-compatible)
    context      LLM-optimized narrative for prompt injection (~1000 tokens)

RELATED COMMANDS:
    zdrive pane log <PANE> <SUMMARY>  Add new entries
    zdrive list                       View all panes"
    )]
    History {
        /// Pane name to view history for
        #[arg(help = "Name of the pane to view history for")]
        name: String,

        /// Limit the number of entries shown
        ///
        /// By default, shows up to 100 most recent entries.
        #[arg(short = 'n', long = "last",
              help = "Show only the last N entries (default: all, up to 100)")]
        last: Option<usize>,

        /// Filter by entry type
        ///
        /// Show only entries of a specific type (milestone, checkpoint, exploration).
        /// Useful for agents to focus on major progress points.
        #[arg(short = 't', long = "type", value_enum,
              help = "Filter by entry type: milestone, checkpoint, or exploration")]
        entry_type: Option<crate::types::IntentType>,

        /// Choose the output format
        ///
        /// Use 'text' for reading, 'json' for tooling, 'context' for agents.
        #[arg(short = 'f', long, default_value = "text", value_enum,
              help = "Output format: text, json, json-compact, markdown, or context")]
        format: OutputFormat,
    },
}

#[derive(Args)]
pub struct TabArgs {
    #[command(subcommand)]
    pub action: Option<TabAction>,
    /// Tab name (used when no subcommand provided, for backwards compatibility)
    pub name: Option<String>,
}

#[derive(Subcommand)]
pub enum TabAction {
    /// Create a new tab with optional correlation ID for event traceability
    ///
    /// Creates a named tab in Zellij with optional correlation ID suffix.
    /// The correlation ID enables tracing work back to triggering events.
    #[command(
        after_help = "EXAMPLES:
    # Create a simple tab
    znav tab create myapp

    # Create tab with correlation ID for PR tracking
    znav tab create \"myapp(fixes)\" --correlation-id pr-42
    # Creates tab named \"myapp(fixes)-pr-42\"

    # Create tab with metadata
    znav tab create debug-session --correlation-id issue-123 --meta project=perth

CORRELATION IDS:
    Correlation IDs link tabs to events from external systems like Bloodbank.
    This enables end-to-end traceability in agentic workflows.

RELATED COMMANDS:
    znav list               View all tabs with correlation IDs
    znav pane batch         Create multiple panes in a tab"
    )]
    Create {
        /// Name for the new tab
        #[arg(help = "Tab name (e.g., 'myapp(fixes)')")]
        name: String,

        /// Correlation ID for event traceability
        ///
        /// Links this tab to a triggering event (e.g., Bloodbank event, PR number).
        /// Tab will be created with name format: {name}-{correlation_id}
        #[arg(short = 'c', long = "correlation-id",
              help = "Correlation ID for tracing (e.g., 'pr-42', 'issue-123')")]
        correlation_id: Option<String>,

        /// Additional metadata key=value pairs
        #[arg(long = "meta", value_parser = parse_key_val,
              help = "Metadata as key=value pairs")]
        meta: Vec<(String, String)>,
    },

    /// Get info about a tab
    Info {
        /// Tab name to get info for
        name: String,
    },
}

pub fn command_name() -> String {
    std::env::args()
        .next()
        .and_then(|arg| {
            std::path::Path::new(&arg)
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
        })
        .unwrap_or_else(|| "zellij-driver".to_string())
}

pub fn collect_meta(pairs: Vec<(String, String)>) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (key, value) in pairs {
        map.insert(key, value);
    }
    map
}

fn parse_key_val(input: &str) -> Result<(String, String), String> {
    let mut parts = input.splitn(2, '=');
    let key = parts
        .next()
        .ok_or_else(|| "meta must be key=value".to_string())?
        .trim();
    let value = parts
        .next()
        .ok_or_else(|| "meta must be key=value".to_string())?
        .trim();

    if key.is_empty() {
        return Err("meta key cannot be empty".to_string());
    }

    Ok((key.to_string(), value.to_string()))
}
