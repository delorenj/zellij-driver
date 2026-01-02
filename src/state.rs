use crate::types::PaneRecord;
use anyhow::{Context, Result};
use chrono::Utc;
use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use redis::AsyncIter;
use std::collections::HashMap;

const META_PREFIX: &str = "meta:";

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
}

fn pane_key(pane_name: &str) -> String {
    format!("znav:pane:{}", pane_name)
}
