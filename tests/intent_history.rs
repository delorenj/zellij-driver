//! Integration tests for Perth intent history functionality.
//!
//! Requires Redis to be running. Tests use unique key prefixes to avoid conflicts.

use anyhow::Result;
use zellij_driver::state::StateManager;
use zellij_driver::types::{IntentEntry, IntentSource, IntentType};

/// Generate a unique test pane name to avoid conflicts between tests
fn test_pane_name(test_name: &str) -> String {
    format!("test_{}_{}", test_name, std::process::id())
}

/// Get Redis URL from environment or use default
fn redis_url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379".to_string())
}

#[tokio::test]
async fn test_log_and_retrieve_single_intent() -> Result<()> {
    let mut state = StateManager::new(&redis_url()).await?;
    let pane_name = test_pane_name("single");

    // Clean up any prior test data
    state.clear_history(&pane_name).await?;

    // Log an intent
    let entry = IntentEntry::new("Implementing STORY-002 Redis schema")
        .with_type(IntentType::Milestone)
        .with_source(IntentSource::Manual);

    state.log_intent(&pane_name, &entry).await?;

    // Retrieve history
    let history = state.get_history(&pane_name, None).await?;
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].summary, "Implementing STORY-002 Redis schema");
    assert_eq!(history[0].entry_type, IntentType::Milestone);
    assert_eq!(history[0].source, IntentSource::Manual);
    assert_eq!(history[0].id, entry.id);

    // Clean up
    state.clear_history(&pane_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_history_ordering_newest_first() -> Result<()> {
    let mut state = StateManager::new(&redis_url()).await?;
    let pane_name = test_pane_name("ordering");

    state.clear_history(&pane_name).await?;

    // Log multiple entries
    let entry1 = IntentEntry::new("First entry");
    state.log_intent(&pane_name, &entry1).await?;

    // Small delay to ensure different timestamps
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let entry2 = IntentEntry::new("Second entry");
    state.log_intent(&pane_name, &entry2).await?;

    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

    let entry3 = IntentEntry::new("Third entry");
    state.log_intent(&pane_name, &entry3).await?;

    // Retrieve - should be newest first
    let history = state.get_history(&pane_name, None).await?;
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].summary, "Third entry");
    assert_eq!(history[1].summary, "Second entry");
    assert_eq!(history[2].summary, "First entry");

    state.clear_history(&pane_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_history_limit() -> Result<()> {
    let mut state = StateManager::new(&redis_url()).await?;
    let pane_name = test_pane_name("limit");

    state.clear_history(&pane_name).await?;

    // Log 5 entries
    for i in 1..=5 {
        let entry = IntentEntry::new(format!("Entry {}", i));
        state.log_intent(&pane_name, &entry).await?;
    }

    // Retrieve with limit of 3
    let history = state.get_history(&pane_name, Some(3)).await?;
    assert_eq!(history.len(), 3);
    assert_eq!(history[0].summary, "Entry 5"); // newest
    assert_eq!(history[1].summary, "Entry 4");
    assert_eq!(history[2].summary, "Entry 3");

    state.clear_history(&pane_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_history_count() -> Result<()> {
    let mut state = StateManager::new(&redis_url()).await?;
    let pane_name = test_pane_name("count");

    state.clear_history(&pane_name).await?;

    assert_eq!(state.get_history_count(&pane_name).await?, 0);

    // Log entries
    for i in 1..=7 {
        let entry = IntentEntry::new(format!("Entry {}", i));
        state.log_intent(&pane_name, &entry).await?;
    }

    assert_eq!(state.get_history_count(&pane_name).await?, 7);

    state.clear_history(&pane_name).await?;
    assert_eq!(state.get_history_count(&pane_name).await?, 0);

    Ok(())
}

#[tokio::test]
async fn test_all_entry_fields_preserved() -> Result<()> {
    let mut state = StateManager::new(&redis_url()).await?;
    let pane_name = test_pane_name("fields");

    state.clear_history(&pane_name).await?;

    let entry = IntentEntry::new("Complex entry with all fields")
        .with_type(IntentType::Exploration)
        .with_source(IntentSource::Agent)
        .with_artifacts(vec![
            "src/state.rs".to_string(),
            "src/types.rs".to_string(),
        ])
        .with_goal_delta("Completed Redis schema implementation")
        .with_commands_run(42);

    state.log_intent(&pane_name, &entry).await?;

    let history = state.get_history(&pane_name, None).await?;
    assert_eq!(history.len(), 1);

    let retrieved = &history[0];
    assert_eq!(retrieved.id, entry.id);
    assert_eq!(retrieved.summary, "Complex entry with all fields");
    assert_eq!(retrieved.entry_type, IntentType::Exploration);
    assert_eq!(retrieved.source, IntentSource::Agent);
    assert_eq!(retrieved.artifacts, vec!["src/state.rs", "src/types.rs"]);
    assert_eq!(retrieved.goal_delta, Some("Completed Redis schema implementation".to_string()));
    assert_eq!(retrieved.commands_run, Some(42));

    state.clear_history(&pane_name).await?;
    Ok(())
}

#[tokio::test]
async fn test_empty_history() -> Result<()> {
    let mut state = StateManager::new(&redis_url()).await?;
    let pane_name = test_pane_name("empty");

    state.clear_history(&pane_name).await?;

    let history = state.get_history(&pane_name, None).await?;
    assert!(history.is_empty());

    let count = state.get_history_count(&pane_name).await?;
    assert_eq!(count, 0);

    Ok(())
}
