use clap::{Args, Parser, Subcommand};
use std::collections::HashMap;

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
    Info { name: String },
}

#[derive(Args)]
pub struct TabArgs {
    pub name: String,
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
