mod cli;
mod config;
mod context;
mod filter;
mod llm;
mod orchestrator;
mod output;
mod state;
mod types;
mod zellij;

use anyhow::{anyhow, Result};
use clap::{CommandFactory, FromArgMatches};
use cli::{collect_meta, command_name, Cli, Command, ConfigAction, OutputFormat, PaneAction, TabAction};
use config::Config;
use orchestrator::Orchestrator;
use output::OutputFormatter;
use state::StateManager;
use types::IntentEntry;
use zellij::ZellijDriver;

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        eprintln!("{err}");
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let name = command_name();
    let name_static: &'static str = Box::leak(name.into_boxed_str());
    let command = Cli::command().name(name_static);
    let matches = command.get_matches();
    let cli = Cli::from_arg_matches(&matches)?;
    let config = Config::load()?;
    let state = StateManager::new(&config.redis_url).await?;
    let zellij = ZellijDriver::new();

    // Check Zellij version for commands that interact with Zellij
    if needs_zellij_check(&cli.command) {
        zellij.check_version().await?;
    }

    let mut orchestrator = Orchestrator::new(state, zellij);

    match cli.command {
        Command::Pane(args) => {
            if let Some(action) = args.action {
                match action {
                    PaneAction::Info { name } => {
                        let info = orchestrator.pane_info(name).await?;
                        let json = serde_json::to_string_pretty(&info)?;
                        println!("{json}");
                        if matches!(info.status, types::PaneStatus::Missing) {
                            std::process::exit(2);
                        }
                        return Ok(());
                    }
                    PaneAction::Log { name, summary, entry_type, source, artifacts } => {
                        // Resolve artifact paths (try absolute, fallback to as-is for non-existent)
                        let resolved_artifacts: Vec<String> = artifacts
                            .into_iter()
                            .map(|p| {
                                std::fs::canonicalize(&p)
                                    .map(|abs| abs.to_string_lossy().to_string())
                                    .unwrap_or(p)
                            })
                            .collect();

                        let entry = IntentEntry::new(&summary)
                            .with_type(entry_type)
                            .with_source(source)
                            .with_artifacts(resolved_artifacts);
                        orchestrator.log_intent(&name, &entry).await?;

                        let artifact_count = entry.artifacts.len();
                        let source_tag = match source {
                            types::IntentSource::Agent => " [agent]",
                            _ => "",
                        };
                        if artifact_count > 0 {
                            println!(
                                "Logged {} for '{}'{}: {} ({} artifact{})",
                                entry.entry_type_str().to_lowercase(),
                                name,
                                source_tag,
                                summary,
                                artifact_count,
                                if artifact_count == 1 { "" } else { "s" }
                            );
                        } else {
                            println!("Logged {} for '{}'{}: {}", entry.entry_type_str().to_lowercase(), name, source_tag, summary);
                        }
                        return Ok(());
                    }
                    PaneAction::History { name, last, format } => {
                        let history = orchestrator.get_history(&name, last).await?;

                        match format {
                            OutputFormat::Json => {
                                let output = serde_json::json!({
                                    "schema_version": "2.0",
                                    "pane": name,
                                    "entries": history,
                                });
                                println!("{}", serde_json::to_string_pretty(&output)?);
                            }
                            OutputFormat::JsonCompact => {
                                let output = serde_json::json!({
                                    "schema_version": "2.0",
                                    "pane": name,
                                    "entries": history,
                                });
                                println!("{}", serde_json::to_string(&output)?);
                            }
                            OutputFormat::Text => {
                                let formatter = OutputFormatter::new();
                                println!("{}", formatter.format_history(&history, &name));
                            }
                            OutputFormat::Markdown => {
                                let formatter = OutputFormatter::new();
                                println!("{}", formatter.format_markdown(&history, &name));
                            }
                            OutputFormat::Context => {
                                let formatter = OutputFormatter::new();
                                println!("{}", formatter.format_context(&history, &name));
                            }
                        }
                        return Ok(());
                    }
                    PaneAction::Snapshot { name } => {
                        let llm_config = config.llm.clone();
                        let consent_given = config.privacy.consent_given;
                        let result = orchestrator.snapshot(&name, &llm_config, consent_given).await?;

                        println!("Generated snapshot for '{}':", name);
                        println!();
                        println!("  Summary: {}", result.summary);
                        println!("  Type: {:?}", result.entry_type);

                        if !result.key_files.is_empty() {
                            println!("  Key files:");
                            for file in &result.key_files {
                                println!("    - {}", file);
                            }
                        }

                        if let Some(tokens) = result.tokens_used {
                            println!("  Tokens used: {}", tokens);
                        }

                        return Ok(());
                    }
                }
            }

            let pane_name = args.name.ok_or_else(|| anyhow!("pane name is required"))?;
            let meta = collect_meta(args.meta);
            let show_last_intent = config.display.show_last_intent;
            orchestrator
                .open_pane(pane_name, args.tab, args.session, meta, show_last_intent)
                .await?;
        }
        Command::Tab(args) => {
            match args.action {
                Some(TabAction::Create { name, correlation_id, meta }) => {
                    let meta_map = collect_meta(meta);
                    let result = orchestrator.create_tab(name, correlation_id, meta_map).await?;

                    if result.created {
                        print!("Created tab '{}'", result.tab_name);
                    } else {
                        print!("Focused existing tab '{}'", result.tab_name);
                    }

                    if let Some(ref id) = result.correlation_id {
                        print!(" (correlation: {})", id);
                    }

                    println!(" in session '{}'", result.session);
                }
                Some(TabAction::Info { name }) => {
                    match orchestrator.tab_info(&name).await? {
                        Some(tab) => {
                            let json = serde_json::to_string_pretty(&tab)?;
                            println!("{}", json);
                        }
                        None => {
                            eprintln!("Tab '{}' not found in Redis", name);
                            std::process::exit(2);
                        }
                    }
                }
                None => {
                    // Backwards compatibility: just ensure the tab exists
                    let tab_name = args.name.ok_or_else(|| anyhow!("tab name is required"))?;
                    let created = orchestrator.ensure_tab(&tab_name).await?;
                    if created {
                        println!("Created tab '{}'", tab_name);
                    } else {
                        println!("Focused tab '{}'", tab_name);
                    }
                }
            }
        }
        Command::Reconcile => {
            orchestrator.reconcile().await?;
        }
        Command::List => {
            orchestrator.visualize().await?;
        }
        Command::Config(args) => {
            match args.action {
                ConfigAction::Show => {
                    println!("{}", config.display());
                }
                ConfigAction::Set { key, value } => {
                    let old_value = Config::set_value(&key, &value)?;

                    match old_value {
                        Some(old) => {
                            println!("Updated '{}': '{}' -> '{}'", key, old, value);
                        }
                        None => {
                            println!("Set '{}': '{}'", key, value);
                        }
                    }
                }
                ConfigAction::Consent { grant, revoke } => {
                    if grant {
                        Config::grant_consent()?;
                        println!("Consent granted for LLM data sharing.");
                        println!();
                        println!("The snapshot command will now send the following to your configured LLM:");
                        println!("  - Recent shell commands");
                        println!("  - Git diff showing recent changes");
                        println!("  - Names of recently modified files");
                        println!();
                        println!("Secrets (API keys, passwords) are automatically filtered.");
                        println!("You can revoke consent at any time with: zdrive config consent --revoke");
                    } else if revoke {
                        Config::revoke_consent()?;
                        println!("Consent revoked. The snapshot command will no longer send data to LLM providers.");
                    } else {
                        // Neither flag provided - show current status
                        if config.privacy.consent_given {
                            println!("Consent status: GRANTED");
                            if let Some(ref ts) = config.privacy.consent_timestamp {
                                println!("Granted at: {}", ts);
                            }
                        } else {
                            println!("Consent status: NOT GRANTED");
                            println!();
                            println!("To use the snapshot command, you must grant consent:");
                            println!("  zdrive config consent --grant");
                        }
                    }
                }
            }
        }
        Command::Migrate(args) => {
            let result = orchestrator.migrate_keyspace(args.dry_run).await?;

            if args.dry_run {
                println!("=== DRY RUN (no changes made) ===\n");
            }

            println!("Migration Summary:");
            println!("  Total keys found: {}", result.total_keys);
            println!("  Migrated: {}", result.migrated_count);
            println!("  Skipped: {}", result.skipped_count);
            println!("  Errors: {}", result.error_count);

            if !result.would_migrate.is_empty() {
                println!("\nWould migrate:");
                for m in &result.would_migrate {
                    println!("  {}", m);
                }
            }

            if !result.migrated.is_empty() {
                println!("\nMigrated:");
                for m in &result.migrated {
                    println!("  {}", m);
                }
            }

            if !result.skipped.is_empty() {
                println!("\nSkipped:");
                for s in &result.skipped {
                    println!("  {}", s);
                }
            }

            if !result.errors.is_empty() {
                println!("\nErrors:");
                for e in &result.errors {
                    eprintln!("  {}", e);
                }
                std::process::exit(1);
            }
        }
    }

    Ok(())
}

/// Determines if a command needs Zellij version check.
/// Commands that only interact with Redis don't need Zellij.
fn needs_zellij_check(command: &Command) -> bool {
    match command {
        // These commands interact with Zellij
        Command::Pane(args) => {
            // Pane subcommands that only use Redis or LLM
            match &args.action {
                Some(PaneAction::Log { .. }) => false,
                Some(PaneAction::History { .. }) => false,
                Some(PaneAction::Snapshot { .. }) => false, // Uses Redis + LLM, not Zellij
                Some(PaneAction::Info { .. }) => true, // Checks pane status via Zellij
                None => true, // Opening a pane requires Zellij
            }
        }
        Command::Tab(args) => {
            // Tab info only uses Redis
            match &args.action {
                Some(TabAction::Info { .. }) => false,
                Some(TabAction::Create { .. }) => true, // Creating requires Zellij
                None => true, // Ensuring tab exists requires Zellij
            }
        }
        Command::Reconcile => true,
        Command::List => true,
        // These commands only use Redis or local config
        Command::Migrate(_) => false,
        Command::Config(_) => false,
    }
}
