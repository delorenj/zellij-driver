use crate::llm::LLMConfig;
use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use toml_edit::{DocumentMut, value};

const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1:6379/";
const DEFAULT_AMQP_URL: &str = "amqp://127.0.0.1:5672/%2f";
const DEFAULT_BLOODBANK_EXCHANGE: &str = "bloodbank.events";

#[derive(Debug, Clone)]
pub struct Config {
    pub redis_url: String,
    pub llm: LLMConfig,
    pub privacy: PrivacyConfig,
    pub display: DisplayConfig,
    pub bloodbank: BloodbankConfig,
    pub tab: TabConfig,
}

#[derive(Debug, Clone)]
pub struct DisplayConfig {
    /// Show last intent when resuming a pane
    pub show_last_intent: bool,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            show_last_intent: true, // Enabled by default
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct PrivacyConfig {
    /// Whether user has consented to sending data to LLM
    pub consent_given: bool,
    /// When consent was given (if at all)
    pub consent_timestamp: Option<String>,
}

/// Configuration for Bloodbank event publishing (STORY-026)
#[derive(Debug, Clone)]
pub struct BloodbankConfig {
    /// Whether Bloodbank integration is enabled
    pub enabled: bool,
    /// AMQP URL for RabbitMQ connection
    pub amqp_url: String,
    /// Exchange name for publishing events
    pub exchange: String,
    /// Routing key prefix for events (default: "perth")
    pub routing_key_prefix: String,
}

impl Default for BloodbankConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Disabled by default for graceful degradation
            amqp_url: DEFAULT_AMQP_URL.to_string(),
            exchange: DEFAULT_BLOODBANK_EXCHANGE.to_string(),
            routing_key_prefix: "perth".to_string(),
        }
    }
}

/// Configuration for tab naming conventions (STORY-039)
#[derive(Debug, Clone)]
pub struct TabConfig {
    /// Regex pattern for valid tab names
    /// Default: `^[a-zA-Z0-9_-]+\([a-zA-Z0-9_-]+\)$` matches `repo(context)` format
    pub naming_pattern: String,
}

impl Default for TabConfig {
    fn default() -> Self {
        Self {
            // Pattern matches: name(context) format, e.g., "myapp(fixes)", "perth(dev)"
            naming_pattern: r"^[a-zA-Z0-9_-]+\([a-zA-Z0-9_-]+\)$".to_string(),
        }
    }
}

impl TabConfig {
    /// Check if a tab name matches the naming convention
    pub fn validate_name(&self, name: &str) -> bool {
        regex::Regex::new(&self.naming_pattern)
            .map(|re| re.is_match(name))
            .unwrap_or(false)
    }

    /// Get a human-readable description of the expected format
    pub fn format_hint(&self) -> &'static str {
        "name(context) - e.g., 'myapp(fixes)', 'perth(dev)'"
    }
}

#[derive(Debug, Deserialize, Default)]
struct FileConfig {
    redis_url: Option<String>,
    #[serde(default)]
    llm: LLMConfigFile,
    #[serde(default)]
    privacy: PrivacyConfigFile,
    #[serde(default)]
    display: DisplayConfigFile,
    #[serde(default)]
    bloodbank: BloodbankConfigFile,
    #[serde(default)]
    tab: TabConfigFile,
}

#[derive(Debug, Deserialize, Default)]
struct LLMConfigFile {
    provider: Option<String>,
    anthropic_api_key: Option<String>,
    openai_api_key: Option<String>,
    ollama_url: Option<String>,
    model: Option<String>,
    max_tokens: Option<u32>,
}

#[derive(Debug, Deserialize, Default)]
struct PrivacyConfigFile {
    consent_given: Option<bool>,
    consent_timestamp: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct DisplayConfigFile {
    show_last_intent: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
struct BloodbankConfigFile {
    enabled: Option<bool>,
    amqp_url: Option<String>,
    exchange: Option<String>,
    routing_key_prefix: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
struct TabConfigFile {
    naming_pattern: Option<String>,
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = Self::path();
        if !path.exists() {
            return Ok(Self::default());
        }

        let contents = fs::read_to_string(&path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;
        let file_config: FileConfig = toml::from_str(&contents)
            .with_context(|| format!("failed to parse config file: {}", path.display()))?;

        Ok(Self {
            redis_url: file_config
                .redis_url
                .unwrap_or_else(|| DEFAULT_REDIS_URL.to_string()),
            llm: LLMConfig {
                provider: file_config.llm.provider.unwrap_or_else(|| "none".to_string()),
                anthropic_api_key: file_config.llm.anthropic_api_key,
                openai_api_key: file_config.llm.openai_api_key,
                ollama_url: file_config.llm.ollama_url.unwrap_or_else(|| "http://localhost:11434".to_string()),
                model: file_config.llm.model,
                max_tokens: file_config.llm.max_tokens.unwrap_or(1024),
            },
            privacy: PrivacyConfig {
                consent_given: file_config.privacy.consent_given.unwrap_or(false),
                consent_timestamp: file_config.privacy.consent_timestamp,
            },
            display: DisplayConfig {
                show_last_intent: file_config.display.show_last_intent.unwrap_or(true),
            },
            bloodbank: BloodbankConfig {
                enabled: file_config.bloodbank.enabled.unwrap_or(false),
                amqp_url: file_config.bloodbank.amqp_url.unwrap_or_else(|| DEFAULT_AMQP_URL.to_string()),
                exchange: file_config.bloodbank.exchange.unwrap_or_else(|| DEFAULT_BLOODBANK_EXCHANGE.to_string()),
                routing_key_prefix: file_config.bloodbank.routing_key_prefix.unwrap_or_else(|| "perth".to_string()),
            },
            tab: TabConfig {
                naming_pattern: file_config.tab.naming_pattern.unwrap_or_else(|| TabConfig::default().naming_pattern),
            },
        })
    }

    /// Returns the path to the configuration file.
    pub fn path() -> PathBuf {
        if let Ok(dir) = env::var("XDG_CONFIG_HOME") {
            return Path::new(&dir).join("zellij-driver").join("config.toml");
        }

        let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Path::new(&home)
            .join(".config")
            .join("zellij-driver")
            .join("config.toml")
    }

    /// Display configuration with default indication.
    pub fn display(&self) -> String {
        let path = Self::path();
        let file_exists = path.exists();

        let mut lines = Vec::new();

        lines.push(format!("Config file: {}", path.display()));
        lines.push(format!(
            "Status: {}",
            if file_exists { "loaded" } else { "using defaults" }
        ));
        lines.push(String::new());
        lines.push("Settings:".to_string());

        // Mask sensitive parts of Redis URL (passwords)
        let masked_redis = mask_redis_url(&self.redis_url);
        let is_default = self.redis_url == DEFAULT_REDIS_URL;
        lines.push(format!(
            "  redis_url: {}{}",
            masked_redis,
            if is_default { " (default)" } else { "" }
        ));

        // LLM settings
        lines.push(String::new());
        lines.push("LLM Settings:".to_string());
        lines.push(format!(
            "  provider: {}{}",
            self.llm.provider,
            if self.llm.provider == "none" { " (default)" } else { "" }
        ));

        // Show API key status (masked)
        if let Some(ref key) = self.llm.anthropic_api_key {
            lines.push(format!("  anthropic_api_key: {}***", &key[..key.len().min(8)]));
        } else if env::var("ANTHROPIC_API_KEY").is_ok() {
            lines.push("  anthropic_api_key: (from environment)".to_string());
        }

        if let Some(ref key) = self.llm.openai_api_key {
            lines.push(format!("  openai_api_key: {}***", &key[..key.len().min(8)]));
        } else if env::var("OPENAI_API_KEY").is_ok() {
            lines.push("  openai_api_key: (from environment)".to_string());
        }

        if self.llm.provider == "ollama" || self.llm.ollama_url != "http://localhost:11434" {
            lines.push(format!("  ollama_url: {}", self.llm.ollama_url));
        }

        if let Some(ref model) = self.llm.model {
            lines.push(format!("  model: {}", model));
        }

        lines.push(format!("  max_tokens: {}", self.llm.max_tokens));

        // Privacy settings
        lines.push(String::new());
        lines.push("Privacy Settings:".to_string());
        lines.push(format!(
            "  consent_given: {}",
            if self.privacy.consent_given { "yes" } else { "no" }
        ));
        if let Some(ref ts) = self.privacy.consent_timestamp {
            lines.push(format!("  consent_timestamp: {}", ts));
        }

        // Display settings
        lines.push(String::new());
        lines.push("Display Settings:".to_string());
        lines.push(format!(
            "  show_last_intent: {}{}",
            if self.display.show_last_intent { "yes" } else { "no" },
            if self.display.show_last_intent { " (default)" } else { "" }
        ));

        // Bloodbank settings
        lines.push(String::new());
        lines.push("Bloodbank Settings:".to_string());
        lines.push(format!(
            "  enabled: {}{}",
            if self.bloodbank.enabled { "yes" } else { "no" },
            if !self.bloodbank.enabled { " (default)" } else { "" }
        ));
        if self.bloodbank.enabled || self.bloodbank.amqp_url != DEFAULT_AMQP_URL {
            // Mask password in AMQP URL
            let masked_amqp = mask_amqp_url(&self.bloodbank.amqp_url);
            lines.push(format!(
                "  amqp_url: {}{}",
                masked_amqp,
                if self.bloodbank.amqp_url == DEFAULT_AMQP_URL { " (default)" } else { "" }
            ));
        }
        if self.bloodbank.enabled || self.bloodbank.exchange != DEFAULT_BLOODBANK_EXCHANGE {
            lines.push(format!(
                "  exchange: {}{}",
                self.bloodbank.exchange,
                if self.bloodbank.exchange == DEFAULT_BLOODBANK_EXCHANGE { " (default)" } else { "" }
            ));
        }
        if self.bloodbank.enabled || self.bloodbank.routing_key_prefix != "perth" {
            lines.push(format!(
                "  routing_key_prefix: {}{}",
                self.bloodbank.routing_key_prefix,
                if self.bloodbank.routing_key_prefix == "perth" { " (default)" } else { "" }
            ));
        }

        lines.join("\n")
    }

    /// Set a configuration value and persist to file.
    /// Returns the old value if it was set.
    pub fn set_value(key: &str, new_value: &str) -> Result<Option<String>> {
        // Parse key for nested values (e.g., "llm.provider")
        let parts: Vec<&str> = key.split('.').collect();

        // Validate the key
        let valid_llm_keys = ["provider", "anthropic_api_key", "openai_api_key", "ollama_url", "model", "max_tokens"];
        let valid_privacy_keys = ["consent_given", "consent_timestamp"];
        let valid_display_keys = ["show_last_intent"];
        let valid_bloodbank_keys = ["enabled", "amqp_url", "exchange", "routing_key_prefix"];

        match parts.as_slice() {
            [top_key] if *top_key == "redis_url" => {}
            ["llm", sub_key] if valid_llm_keys.contains(sub_key) => {}
            ["privacy", sub_key] if valid_privacy_keys.contains(sub_key) => {}
            ["display", sub_key] if valid_display_keys.contains(sub_key) => {}
            ["bloodbank", sub_key] if valid_bloodbank_keys.contains(sub_key) => {}
            _ => {
                return Err(anyhow!(
                    "Unknown configuration key: '{}'\nValid keys: redis_url, llm.*, privacy.*, display.*, bloodbank.*",
                    key
                ));
            }
        }

        // Validate the value based on key
        if key == "redis_url" {
            if !new_value.starts_with("redis://") && !new_value.starts_with("rediss://") {
                return Err(anyhow!(
                    "Invalid Redis URL: must start with 'redis://' or 'rediss://'"
                ));
            }
        } else if key == "llm.provider" {
            let valid_providers = ["none", "anthropic", "openai", "ollama"];
            if !valid_providers.contains(&new_value) {
                return Err(anyhow!(
                    "Invalid LLM provider: '{}'\nValid providers: {}",
                    new_value,
                    valid_providers.join(", ")
                ));
            }
        } else if key == "llm.max_tokens" {
            if new_value.parse::<u32>().is_err() {
                return Err(anyhow!("Invalid max_tokens: must be a positive integer"));
            }
        } else if key == "privacy.consent_given" || key == "display.show_last_intent" || key == "bloodbank.enabled" {
            if !["true", "false", "yes", "no"].contains(&new_value.to_lowercase().as_str()) {
                return Err(anyhow!("Invalid {}: must be true/false or yes/no", key.split('.').last().unwrap()));
            }
        } else if key == "bloodbank.amqp_url" {
            if !new_value.starts_with("amqp://") && !new_value.starts_with("amqps://") {
                return Err(anyhow!(
                    "Invalid AMQP URL: must start with 'amqp://' or 'amqps://'"
                ));
            }
        }

        let path = Self::path();
        let old_value: Option<String>;

        // Load existing config or create new document
        let mut doc: DocumentMut = if path.exists() {
            let contents = fs::read_to_string(&path)
                .with_context(|| format!("failed to read config file: {}", path.display()))?;
            contents.parse().with_context(|| {
                format!("failed to parse config file: {}", path.display())
            })?
        } else {
            DocumentMut::new()
        };

        // Get old value and set new value based on key structure
        match parts.as_slice() {
            [top_key] => {
                old_value = doc.get(*top_key).and_then(|v| v.as_str()).map(|s| s.to_string());
                doc[*top_key] = value(new_value);
            }
            ["llm", sub_key] => {
                // Ensure [llm] table exists
                if !doc.contains_key("llm") {
                    doc["llm"] = toml_edit::Item::Table(toml_edit::Table::new());
                }
                old_value = doc["llm"]
                    .get(*sub_key)
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string());
                doc["llm"][*sub_key] = value(new_value);
            }
            ["privacy", sub_key] => {
                // Ensure [privacy] table exists
                if !doc.contains_key("privacy") {
                    doc["privacy"] = toml_edit::Item::Table(toml_edit::Table::new());
                }
                old_value = doc["privacy"]
                    .get(*sub_key)
                    .and_then(|v| v.as_str().or_else(|| v.as_bool().map(|b| if b { "true" } else { "false" })))
                    .map(|s| s.to_string());
                // Handle boolean conversion for consent_given
                if *sub_key == "consent_given" {
                    let bool_val = matches!(new_value.to_lowercase().as_str(), "true" | "yes");
                    doc["privacy"][*sub_key] = toml_edit::value(bool_val);
                } else {
                    doc["privacy"][*sub_key] = value(new_value);
                }
            }
            ["display", sub_key] => {
                // Ensure [display] table exists
                if !doc.contains_key("display") {
                    doc["display"] = toml_edit::Item::Table(toml_edit::Table::new());
                }
                old_value = doc["display"]
                    .get(*sub_key)
                    .and_then(|v| v.as_str().or_else(|| v.as_bool().map(|b| if b { "true" } else { "false" })))
                    .map(|s| s.to_string());
                // Handle boolean conversion for show_last_intent
                if *sub_key == "show_last_intent" {
                    let bool_val = matches!(new_value.to_lowercase().as_str(), "true" | "yes");
                    doc["display"][*sub_key] = toml_edit::value(bool_val);
                } else {
                    doc["display"][*sub_key] = value(new_value);
                }
            }
            ["bloodbank", sub_key] => {
                // Ensure [bloodbank] table exists
                if !doc.contains_key("bloodbank") {
                    doc["bloodbank"] = toml_edit::Item::Table(toml_edit::Table::new());
                }
                old_value = doc["bloodbank"]
                    .get(*sub_key)
                    .and_then(|v| v.as_str().or_else(|| v.as_bool().map(|b| if b { "true" } else { "false" })))
                    .map(|s| s.to_string());
                // Handle boolean conversion for enabled
                if *sub_key == "enabled" {
                    let bool_val = matches!(new_value.to_lowercase().as_str(), "true" | "yes");
                    doc["bloodbank"][*sub_key] = toml_edit::value(bool_val);
                } else {
                    doc["bloodbank"][*sub_key] = value(new_value);
                }
            }
            _ => unreachable!(),
        }

        // Ensure config directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create config directory: {}", parent.display()))?;
        }

        // Write config file
        fs::write(&path, doc.to_string())
            .with_context(|| format!("failed to write config file: {}", path.display()))?;

        Ok(old_value)
    }

    /// Grant consent for LLM data sharing.
    pub fn grant_consent() -> Result<()> {
        let timestamp = chrono::Utc::now().to_rfc3339();
        Self::set_value("privacy.consent_given", "true")?;
        Self::set_value("privacy.consent_timestamp", &timestamp)?;
        Ok(())
    }

    /// Revoke consent for LLM data sharing.
    pub fn revoke_consent() -> Result<()> {
        Self::set_value("privacy.consent_given", "false")?;
        Ok(())
    }
}

/// Mask password in Redis URL for display.
fn mask_redis_url(url: &str) -> String {
    // Redis URLs can be: redis://[:password@]host[:port]/[database]
    // or: redis://user:password@host:port/database
    if let Some(at_pos) = url.find('@') {
        if let Some(proto_end) = url.find("://") {
            let auth_part = &url[proto_end + 3..at_pos];
            if auth_part.contains(':') || !auth_part.is_empty() {
                // Has auth, mask it
                let proto = &url[..proto_end + 3];
                let rest = &url[at_pos..];
                return format!("{}***{}", proto, rest);
            }
        }
    }
    url.to_string()
}

/// Mask password in AMQP URL for display.
fn mask_amqp_url(url: &str) -> String {
    // AMQP URLs: amqp://user:password@host:port/vhost
    if let Some(at_pos) = url.find('@') {
        if let Some(proto_end) = url.find("://") {
            let auth_part = &url[proto_end + 3..at_pos];
            if auth_part.contains(':') || !auth_part.is_empty() {
                // Has auth, mask it
                let proto = &url[..proto_end + 3];
                let rest = &url[at_pos..];
                return format!("{}***{}", proto, rest);
            }
        }
    }
    url.to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            redis_url: DEFAULT_REDIS_URL.to_string(),
            llm: LLMConfig::default(),
            privacy: PrivacyConfig::default(),
            display: DisplayConfig::default(),
            bloodbank: BloodbankConfig::default(),
            tab: TabConfig::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mask_redis_url_no_auth() {
        assert_eq!(
            mask_redis_url("redis://localhost:6379/"),
            "redis://localhost:6379/"
        );
    }

    #[test]
    fn test_mask_redis_url_with_password() {
        assert_eq!(
            mask_redis_url("redis://:mypassword@localhost:6379/0"),
            "redis://***@localhost:6379/0"
        );
    }

    #[test]
    fn test_mask_redis_url_with_user_and_password() {
        assert_eq!(
            mask_redis_url("redis://user:pass@localhost:6379/"),
            "redis://***@localhost:6379/"
        );
    }
}
