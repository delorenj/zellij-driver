use crate::types::{IntentEntry, PaneRecord, TabRecord};
use anyhow::{Context, Result};
use chrono::Utc;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use redis::AsyncIter;
use std::collections::HashMap;

const META_PREFIX: &str = "meta:";
const DEFAULT_HISTORY_LIMIT: usize = 100;

pub struct StateManager {
    conn: MultiplexedConnection,
}

impl StateManager {
    pub async fn new(redis_url: &str) -> Result<Self> {
        let client =
            redis::Client::open(redis_url).context("failed to create redis client")?;
        let conn = client
            .get_multiplexed_tokio_connection()
            .await
            .context("failed to connect to redis")?;
        Ok(Self { conn })
    }

    pub fn now_string() -> String {
        Utc::now().to_rfc3339()
    }

    pub async fn get_pane(&mut self, pane_name: &str) -> Result<Option<PaneRecord>> {
        let key = pane_key(pane_name);
        let map: HashMap<String, String> = self.conn.hgetall(&key).await?;
        if map.is_empty() {
            return Ok(None);
        }

        let mut meta = HashMap::new();
        let mut session = String::new();
        let mut tab = String::new();
        let mut pane_id = None;
        let mut created_at = String::new();
        let mut last_seen = String::new();
        let mut last_accessed = String::new();
        let mut stale = false;

        for (k, v) in map {
            if let Some(meta_key) = k.strip_prefix(META_PREFIX) {
                meta.insert(meta_key.to_string(), v);
                continue;
            }
            match k.as_str() {
                "session" => session = v,
                "tab" => tab = v,
                "pane_id" => pane_id = Some(v),
                "created_at" => created_at = v,
                "last_seen" => last_seen = v,
                "last_accessed" => last_accessed = v,
                "stale" => stale = v == "true",
                _ => {}
            }
        }

        Ok(Some(PaneRecord {
            pane_name: pane_name.to_string(),
            session,
            tab,
            pane_id,
            created_at,
            last_seen,
            last_accessed,
            meta,
            stale,
        }))
    }

    pub async fn upsert_pane(&mut self, record: &PaneRecord) -> Result<()> {
        let key = pane_key(&record.pane_name);
        let mut fields: Vec<(String, String)> = Vec::new();
        fields.push(("session".to_string(), record.session.clone()));
        fields.push(("tab".to_string(), record.tab.clone()));
        fields.push(("created_at".to_string(), record.created_at.clone()));
        fields.push(("last_seen".to_string(), record.last_seen.clone()));
        fields.push((
            "last_accessed".to_string(),
            record.last_accessed.clone(),
        ));
        fields.push(("stale".to_string(), "false".to_string()));

        if let Some(pane_id) = &record.pane_id {
            fields.push(("pane_id".to_string(), pane_id.clone()));
        }

        for (k, v) in &record.meta {
            fields.push((format!("{}{}", META_PREFIX, k), v.clone()));
        }

        let _: () = self.conn.hset_multiple(key, &fields).await?;
        Ok(())
    }

    pub async fn touch_pane(&mut self, pane_name: &str, meta_updates: &HashMap<String, String>) -> Result<()> {
        let key = pane_key(pane_name);
        let now = Self::now_string();
        let mut fields: Vec<(String, String)> = vec![
            ("last_accessed".to_string(), now.clone()),
            ("last_seen".to_string(), now),
            ("stale".to_string(), "false".to_string()),
        ];

        for (k, v) in meta_updates {
            fields.push((format!("{}{}", META_PREFIX, k), v.clone()));
        }

        let _: () = self.conn.hset_multiple(key, &fields).await?;
        Ok(())
    }

    pub async fn mark_seen(&mut self, pane_name: &str) -> Result<()> {
        let key = pane_key(pane_name);
        let now = Self::now_string();
        let fields: Vec<(String, String)> = vec![
            ("last_seen".to_string(), now),
            ("stale".to_string(), "false".to_string()),
        ];
        let _: () = self.conn.hset_multiple(key, &fields).await?;
        Ok(())
    }

    pub async fn mark_stale(&mut self, pane_name: &str) -> Result<()> {
        let key = pane_key(pane_name);
        let _: () = self.conn.hset(key, "stale", "true").await?;
        Ok(())
    }

    pub async fn list_pane_names(&mut self) -> Result<Vec<String>> {
        let mut iter: AsyncIter<String> = self.conn.scan_match("znav:pane:*").await?;
        let mut names = Vec::new();
        while let Some(key) = iter.next_item().await {
            if let Some(name) = key.strip_prefix("znav:pane:") {
                names.push(name.to_string());
            }
        }
        Ok(names)
    }

    pub async fn list_all_panes(&mut self) -> Result<Vec<PaneRecord>> {
        let names = self.list_pane_names().await?;
        let mut panes = Vec::new();
        for name in names {
            if let Some(pane) = self.get_pane(&name).await? {
                panes.push(pane);
            }
        }
        Ok(panes)
    }

    // ========================================================================
    // Intent History Methods (Perth v2.0)
    // ========================================================================

    /// Log an intent entry for a pane.
    /// - LPUSH to history list (newest first)
    /// - Update last_intent on pane hash
    /// - LTRIM to maintain max entries
    pub async fn log_intent(&mut self, pane_name: &str, entry: &IntentEntry) -> Result<()> {
        let history_key = history_key(pane_name);
        let pane_key = pane_key(pane_name);

        // Serialize entry to JSON
        let json = serde_json::to_string(entry)
            .context("failed to serialize IntentEntry")?;

        // LPUSH to add newest entry at head of list
        let _: () = self.conn.lpush(&history_key, &json).await?;

        // Update last_intent summary on pane hash for quick access
        let _: () = self.conn.hset(&pane_key, "last_intent", &entry.summary).await?;
        let _: () = self.conn.hset(&pane_key, "last_intent_at", entry.timestamp.to_rfc3339()).await?;

        // LTRIM to maintain max entries (keep indices 0 to LIMIT-1)
        let _: () = self.conn.ltrim(&history_key, 0, (DEFAULT_HISTORY_LIMIT - 1) as isize).await?;

        Ok(())
    }

    /// Get intent history for a pane.
    /// Returns entries newest-first, up to the specified limit.
    pub async fn get_history(&mut self, pane_name: &str, limit: Option<usize>) -> Result<Vec<IntentEntry>> {
        let history_key = history_key(pane_name);
        let limit = limit.unwrap_or(DEFAULT_HISTORY_LIMIT);

        // LRANGE 0 to (limit-1) gets newest entries
        let entries: Vec<String> = self.conn.lrange(&history_key, 0, (limit - 1) as isize).await?;

        let mut history = Vec::with_capacity(entries.len());
        for json in entries {
            let entry: IntentEntry = serde_json::from_str(&json)
                .context("failed to deserialize IntentEntry from history")?;
            history.push(entry);
        }

        Ok(history)
    }

    /// Get the count of history entries for a pane.
    pub async fn get_history_count(&mut self, pane_name: &str) -> Result<usize> {
        let history_key = history_key(pane_name);
        let count: usize = self.conn.llen(&history_key).await?;
        Ok(count)
    }

    /// Clear all history for a pane.
    pub async fn clear_history(&mut self, pane_name: &str) -> Result<()> {
        let history_key = history_key(pane_name);
        let _: () = self.conn.del(&history_key).await?;
        Ok(())
    }

    // ========================================================================
    // Tab Storage Methods (STORY-036)
    // ========================================================================

    /// Get a tab record by name.
    pub async fn get_tab(&mut self, tab_name: &str, session: &str) -> Result<Option<TabRecord>> {
        let key = tab_key(tab_name, session);
        let map: HashMap<String, String> = self.conn.hgetall(&key).await?;
        if map.is_empty() {
            return Ok(None);
        }

        let mut meta = HashMap::new();
        let mut correlation_id = None;
        let mut created_at = String::new();
        let mut last_accessed = String::new();

        for (k, v) in map {
            if let Some(meta_key) = k.strip_prefix(META_PREFIX) {
                meta.insert(meta_key.to_string(), v);
                continue;
            }
            match k.as_str() {
                "correlation_id" => correlation_id = Some(v),
                "created_at" => created_at = v,
                "last_accessed" => last_accessed = v,
                _ => {}
            }
        }

        Ok(Some(TabRecord {
            tab_name: tab_name.to_string(),
            session: session.to_string(),
            correlation_id,
            created_at,
            last_accessed,
            meta,
        }))
    }

    /// Create or update a tab record.
    pub async fn upsert_tab(&mut self, record: &TabRecord) -> Result<()> {
        let key = tab_key(&record.tab_name, &record.session);
        let mut fields: Vec<(String, String)> = Vec::new();

        fields.push(("created_at".to_string(), record.created_at.clone()));
        fields.push(("last_accessed".to_string(), record.last_accessed.clone()));

        if let Some(correlation_id) = &record.correlation_id {
            fields.push(("correlation_id".to_string(), correlation_id.clone()));
        }

        for (k, v) in &record.meta {
            fields.push((format!("{}{}", META_PREFIX, k), v.clone()));
        }

        let _: () = self.conn.hset_multiple(key, &fields).await?;
        Ok(())
    }

    /// Touch a tab (update last_accessed timestamp).
    pub async fn touch_tab(&mut self, tab_name: &str, session: &str) -> Result<()> {
        let key = tab_key(tab_name, session);
        let now = Self::now_string();
        let _: () = self.conn.hset(&key, "last_accessed", now).await?;
        Ok(())
    }

    /// List all tab names for a session.
    pub async fn list_tab_names(&mut self, session: &str) -> Result<Vec<String>> {
        let pattern = format!("perth:tab:{}:*", session);
        let mut iter: AsyncIter<String> = self.conn.scan_match(&pattern).await?;
        let mut names = Vec::new();
        let prefix = format!("perth:tab:{}:", session);
        while let Some(key) = iter.next_item().await {
            if let Some(name) = key.strip_prefix(&prefix) {
                names.push(name.to_string());
            }
        }
        Ok(names)
    }

    /// List all tabs for a session.
    pub async fn list_tabs(&mut self, session: &str) -> Result<Vec<TabRecord>> {
        let names = self.list_tab_names(session).await?;
        let mut tabs = Vec::new();
        for name in names {
            if let Some(tab) = self.get_tab(&name, session).await? {
                tabs.push(tab);
            }
        }
        Ok(tabs)
    }

    /// Check if a tab exists.
    pub async fn tab_exists(&mut self, tab_name: &str, session: &str) -> Result<bool> {
        let key = tab_key(tab_name, session);
        let exists: bool = self.conn.exists(&key).await?;
        Ok(exists)
    }

    // ========================================================================
    // Migration Methods (v1.0 → v2.0)
    // ========================================================================

    /// Migrate from znav:* to perth:* keyspace.
    /// Returns (migrated_count, skipped_count, error_count).
    pub async fn migrate_keyspace(&mut self, dry_run: bool) -> Result<MigrationResult> {
        let mut result = MigrationResult::default();

        // Scan for znav:pane:* keys (v1.0 pane data)
        // Collect all keys first to release the iterator borrow
        let znav_keys: Vec<String> = {
            let mut iter: AsyncIter<String> = self.conn.scan_match("znav:pane:*").await?;
            let mut keys = Vec::new();
            while let Some(key) = iter.next_item().await {
                // Skip history keys if any exist in v1 format
                if !key.contains(":history") {
                    keys.push(key);
                }
            }
            keys
        };

        result.total_keys = znav_keys.len();

        for old_key in znav_keys {
            // Extract pane name from znav:pane:<name>
            let pane_name = match old_key.strip_prefix("znav:pane:") {
                Some(name) => name.to_string(),
                None => {
                    result.errors.push(format!("Invalid key format: {}", old_key));
                    result.error_count += 1;
                    continue;
                }
            };

            let new_key = format!("perth:pane:{}", pane_name);

            // Check if target key already exists
            let exists: bool = self.conn.exists(&new_key).await?;
            if exists {
                result.skipped.push(format!("{} -> {} (already exists)", old_key, new_key));
                result.skipped_count += 1;
                continue;
            }

            if dry_run {
                result.would_migrate.push(format!("{} -> {}", old_key, new_key));
                result.migrated_count += 1;
            } else {
                // Copy hash data to new key
                let data: HashMap<String, String> = self.conn.hgetall(&old_key).await?;
                if !data.is_empty() {
                    let fields: Vec<(String, String)> = data.into_iter().collect();
                    let _: () = self.conn.hset_multiple(&new_key, &fields).await?;
                    result.migrated.push(format!("{} -> {}", old_key, new_key));
                    result.migrated_count += 1;
                } else {
                    result.skipped.push(format!("{} (empty)", old_key));
                    result.skipped_count += 1;
                }
            }
        }

        Ok(result)
    }

    /// Save a session snapshot to Redis
    pub async fn save_snapshot(&self, snapshot: &crate::types::SessionSnapshot) -> Result<()> {
        let key = snapshot.redis_key();
        let json = serde_json::to_string(snapshot)
            .context("failed to serialize snapshot")?;

        let _: () = self.conn
            .clone()
            .set(&key, json)
            .await
            .context("failed to save snapshot to redis")?;

        Ok(())
    }

    /// List snapshots for a specific session
    pub async fn list_snapshots(&self, session: &str) -> Result<Vec<crate::types::SessionSnapshot>> {
        let pattern = format!("perth:snapshots:{}:*", session);
        let keys: Vec<String> = self.conn
            .clone()
            .keys(&pattern)
            .await
            .context("failed to scan snapshot keys")?;

        let mut snapshots = Vec::new();
        for key in keys {
            if let Ok(json) = self.conn.clone().get::<_, String>(&key).await {
                if let Ok(snapshot) = serde_json::from_str::<crate::types::SessionSnapshot>(&json) {
                    snapshots.push(snapshot);
                }
            }
        }

        // Sort by creation time (newest first)
        snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(snapshots)
    }

    /// List all snapshots across all sessions
    pub async fn list_all_snapshots(&self) -> Result<Vec<crate::types::SessionSnapshot>> {
        let pattern = "perth:snapshots:*";
        let keys: Vec<String> = self.conn
            .clone()
            .keys(pattern)
            .await
            .context("failed to scan snapshot keys")?;

        let mut snapshots = Vec::new();
        for key in keys {
            if let Ok(json) = self.conn.clone().get::<_, String>(&key).await {
                if let Ok(snapshot) = serde_json::from_str::<crate::types::SessionSnapshot>(&json) {
                    snapshots.push(snapshot);
                }
            }
        }

        // Sort by creation time (newest first)
        snapshots.sort_by(|a, b| b.created_at.cmp(&a.created_at));

        Ok(snapshots)
    }

    /// Get a snapshot by name
    pub async fn get_snapshot(&self, session: &str, name: &str) -> Result<crate::types::SessionSnapshot> {
        let key = format!("perth:snapshots:{}:{}", session, name);
        let json: String = self.conn
            .clone()
            .get(&key)
            .await
            .context("snapshot not found")?;

        let snapshot = serde_json::from_str(&json)
            .context("failed to deserialize snapshot")?;

        Ok(snapshot)
    }

    /// Delete a snapshot by name
    pub async fn delete_snapshot(&self, session: &str, name: &str) -> Result<()> {
        let key = format!("perth:snapshots:{}:{}", session, name);
        let _: () = self.conn
            .clone()
            .del(&key)
            .await
            .context("failed to delete snapshot")?;

        Ok(())
    }

    /// Get snapshot ancestry chain (parent, grandparent, etc.)
    ///
    /// Returns snapshots from newest to oldest, stopping when parent_id is None
    /// or when a parent cannot be found.
    pub async fn get_snapshot_ancestry(&self, session: &str, name: &str) -> Result<Vec<crate::types::SessionSnapshot>> {
        let mut ancestry = Vec::new();
        let mut current = self.get_snapshot(session, name).await?;

        ancestry.push(current.clone());

        // Walk up the parent chain
        while let Some(parent_id) = current.parent_id {
            // Find parent by ID (need to scan all snapshots in session)
            let snapshots = self.list_snapshots(session).await?;

            match snapshots.into_iter().find(|s| s.id == parent_id) {
                Some(parent) => {
                    ancestry.push(parent.clone());
                    current = parent;
                }
                None => {
                    // Parent not found, stop traversal
                    break;
                }
            }
        }

        Ok(ancestry)
    }
}

/// Result of a keyspace migration operation.
#[derive(Debug, Default)]
pub struct MigrationResult {
    pub total_keys: usize,
    pub migrated_count: usize,
    pub skipped_count: usize,
    pub error_count: usize,
    pub migrated: Vec<String>,
    pub skipped: Vec<String>,
    pub would_migrate: Vec<String>,
    pub errors: Vec<String>,
}

fn pane_key(pane_name: &str) -> String {
    format!("znav:pane:{}", pane_name)
}

fn history_key(pane_name: &str) -> String {
    format!("perth:pane:{}:history", pane_name)
}

fn tab_key(tab_name: &str, session: &str) -> String {
    format!("perth:tab:{}:{}", session, tab_name)
}
