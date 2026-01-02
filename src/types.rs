use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct PaneRecord {
    pub pane_name: String,
    pub session: String,
    pub tab: String,
    pub pane_id: Option<String>,
    pub created_at: String,
    pub last_seen: String,
    pub last_accessed: String,
    pub meta: HashMap<String, String>,
    pub stale: bool,
}

impl PaneRecord {
    pub fn new(
        pane_name: String,
        session: String,
        tab: String,
        now: String,
        meta: HashMap<String, String>,
    ) -> Self {
        Self {
            pane_name,
            session,
            tab,
            pane_id: None,
            created_at: now.clone(),
            last_seen: now.clone(),
            last_accessed: now,
            meta,
            stale: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PaneStatus {
    Found,
    Stale,
    Missing,
}

#[derive(Debug, Clone, Serialize)]
pub struct PaneInfoOutput {
    pub pane_name: String,
    pub session: String,
    pub tab: String,
    pub pane_id: Option<String>,
    pub created_at: String,
    pub last_seen: String,
    pub last_accessed: String,
    pub meta: HashMap<String, String>,
    pub status: PaneStatus,
    pub source: String,
}

impl PaneInfoOutput {
    pub fn missing(pane_name: String) -> Self {
        Self {
            pane_name,
            session: String::new(),
            tab: String::new(),
            pane_id: None,
            created_at: String::new(),
            last_seen: String::new(),
            last_accessed: String::new(),
            meta: HashMap::new(),
            status: PaneStatus::Missing,
            source: "redis".to_string(),
        }
    }
}
