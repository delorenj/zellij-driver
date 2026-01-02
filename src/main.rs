mod cli;
mod config;
mod orchestrator;
mod state;
mod types;
mod zellij;

use anyhow::{anyhow, Result};
use clap::{CommandFactory, FromArgMatches};
use cli::{collect_meta, command_name, Cli, Command, PaneAction};
use config::Config;
use orchestrator::Orchestrator;
use state::StateManager;
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
                }
            }

            let pane_name = args.name.ok_or_else(|| anyhow!("pane name is required"))?;
            let meta = collect_meta(args.meta);
            orchestrator
                .open_pane(pane_name, args.tab, args.session, meta)
                .await?;
        }
        Command::Tab(args) => {
            orchestrator.ensure_tab(&args.name).await?;
        }
        Command::Reconcile => {
            orchestrator.reconcile().await?;
        }
        Command::List => {
            orchestrator.visualize().await?;
        }
    }

    Ok(())
}
