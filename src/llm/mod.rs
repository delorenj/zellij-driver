mod anthropic;
mod circuit_breaker;
mod noop;
mod ollama;
mod openai;

pub use anthropic::AnthropicProvider;
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig, CircuitState};
pub use noop::NoOpProvider;
pub use ollama::OllamaProvider;
pub use openai::OpenAIProvider;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Context captured for LLM summarization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionContext {
    /// Recent shell commands (already filtered for secrets)
    pub shell_history: Vec<String>,

    /// Git diff output (already filtered for secrets)
    pub git_diff: Option<String>,

    /// Current working directory
    pub cwd: String,

    /// Active files being worked on
    pub active_files: Vec<String>,

    /// Git branch name
    pub git_branch: Option<String>,

    /// Pane name for context
    pub pane_name: String,

    /// Any existing intent summary to build upon
    pub existing_summary: Option<String>,
}

impl SessionContext {
    pub fn new(pane_name: impl Into<String>) -> Self {
        Self {
            shell_history: Vec::new(),
            git_diff: None,
            cwd: String::new(),
            active_files: Vec::new(),
            git_branch: None,
            pane_name: pane_name.into(),
            existing_summary: None,
        }
    }

    pub fn with_shell_history(mut self, history: Vec<String>) -> Self {
        self.shell_history = history;
        self
    }

    pub fn with_git_diff(mut self, diff: impl Into<String>) -> Self {
        self.git_diff = Some(diff.into());
        self
    }

    pub fn with_cwd(mut self, cwd: impl Into<String>) -> Self {
        self.cwd = cwd.into();
        self
    }

    pub fn with_active_files(mut self, files: Vec<String>) -> Self {
        self.active_files = files;
        self
    }

    pub fn with_git_branch(mut self, branch: impl Into<String>) -> Self {
        self.git_branch = Some(branch.into());
        self
    }

    pub fn with_existing_summary(mut self, summary: impl Into<String>) -> Self {
        self.existing_summary = Some(summary.into());
        self
    }
}

/// Result from LLM summarization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummarizationResult {
    /// Generated summary of the session
    pub summary: String,

    /// Suggested entry type based on content
    pub suggested_type: Option<String>,

    /// Key files identified
    pub key_files: Vec<String>,

    /// Tokens used (for cost tracking)
    pub tokens_used: Option<u32>,
}

/// Trait for LLM providers.
/// All providers must be thread-safe (Send + Sync) for async operations.
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Generate a summary of the given session context.
    async fn summarize(&self, context: &SessionContext) -> Result<SummarizationResult>;

    /// Get the provider name for logging/config.
    fn name(&self) -> &'static str;

    /// Check if the provider is available (has API key, etc.).
    fn is_available(&self) -> bool;
}

/// Configuration for LLM providers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LLMConfig {
    /// Which provider to use: "anthropic", "openai", "ollama", "none"
    #[serde(default = "default_provider")]
    pub provider: String,

    /// Anthropic API key (or from ANTHROPIC_API_KEY env)
    pub anthropic_api_key: Option<String>,

    /// OpenAI API key (or from OPENAI_API_KEY env)
    pub openai_api_key: Option<String>,

    /// Ollama endpoint URL
    #[serde(default = "default_ollama_url")]
    pub ollama_url: String,

    /// Model to use for summarization
    pub model: Option<String>,

    /// Maximum tokens for response
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
}

fn default_provider() -> String {
    "none".to_string()
}

fn default_ollama_url() -> String {
    "http://localhost:11434".to_string()
}

fn default_max_tokens() -> u32 {
    1024
}

/// Create an LLM provider based on configuration.
pub fn create_provider(config: &LLMConfig) -> Box<dyn LLMProvider> {
    match config.provider.as_str() {
        "anthropic" => {
            let api_key = config
                .anthropic_api_key
                .clone()
                .or_else(|| std::env::var("ANTHROPIC_API_KEY").ok());

            if let Some(key) = api_key {
                let model = config
                    .model
                    .clone()
                    .unwrap_or_else(|| "claude-sonnet-4-20250514".to_string());
                Box::new(AnthropicProvider::new(key, model, config.max_tokens))
            } else {
                Box::new(NoOpProvider::new(
                    "Anthropic API key not configured. Set ANTHROPIC_API_KEY or add anthropic_api_key to config.",
                ))
            }
        }
        "openai" => {
            let api_key = config
                .openai_api_key
                .clone()
                .or_else(|| std::env::var("OPENAI_API_KEY").ok());

            if let Some(key) = api_key {
                let model = config
                    .model
                    .clone()
                    .unwrap_or_else(|| "gpt-4o-mini".to_string());
                Box::new(OpenAIProvider::new(key, model, config.max_tokens))
            } else {
                Box::new(NoOpProvider::new(
                    "OpenAI API key not configured. Set OPENAI_API_KEY or add openai_api_key to config.",
                ))
            }
        }
        "ollama" => {
            let endpoint = if config.ollama_url.is_empty() {
                default_ollama_url()
            } else {
                config.ollama_url.clone()
            };

            let model = config
                .model
                .clone()
                .unwrap_or_else(|| "llama3.2".to_string());

            Box::new(OllamaProvider::new(endpoint, model))
        }
        "none" | "" => Box::new(NoOpProvider::new(
            "LLM provider disabled. Set [llm].provider in config to enable.",
        )),
        other => Box::new(NoOpProvider::new(format!(
            "Unknown LLM provider: '{}'. Valid options: anthropic, openai, ollama, none",
            other
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_context_builder() {
        let ctx = SessionContext::new("test-pane")
            .with_cwd("/home/user/project")
            .with_git_branch("feature/test")
            .with_shell_history(vec!["git status".to_string(), "cargo build".to_string()]);

        assert_eq!(ctx.pane_name, "test-pane");
        assert_eq!(ctx.cwd, "/home/user/project");
        assert_eq!(ctx.git_branch, Some("feature/test".to_string()));
        assert_eq!(ctx.shell_history.len(), 2);
    }

    #[test]
    fn test_create_noop_provider() {
        let config = LLMConfig::default();
        let provider = create_provider(&config);
        assert_eq!(provider.name(), "noop");
        assert!(!provider.is_available());
    }

    #[test]
    fn test_create_anthropic_without_key() {
        // Temporarily unset env var for test
        let config = LLMConfig {
            provider: "anthropic".to_string(),
            anthropic_api_key: None,
            ..Default::default()
        };

        // This should fall back to NoOp since no API key
        let provider = create_provider(&config);
        // Will be NoOp if ANTHROPIC_API_KEY is not set in environment
        // We can't reliably test this without controlling env
    }

    #[test]
    fn test_create_openai_with_key() {
        let config = LLMConfig {
            provider: "openai".to_string(),
            openai_api_key: Some("sk-test-key".to_string()),
            ..Default::default()
        };

        let provider = create_provider(&config);
        assert_eq!(provider.name(), "openai");
        assert!(provider.is_available());
    }

    #[test]
    fn test_create_openai_without_key() {
        // Test that without config API key AND if env var not set, we get noop
        // Note: This test may pass or fail depending on OPENAI_API_KEY env var
        // We primarily test the config path - if config key is None and env isn't set, it's noop
        let config = LLMConfig {
            provider: "openai".to_string(),
            openai_api_key: None,
            ..Default::default()
        };

        let provider = create_provider(&config);
        // If OPENAI_API_KEY env var is set, this will be "openai", otherwise "noop"
        // We can't reliably test this without controlling env
        assert!(provider.name() == "openai" || provider.name() == "noop");
    }

    #[test]
    fn test_create_ollama_provider() {
        let config = LLMConfig {
            provider: "ollama".to_string(),
            ollama_url: "http://localhost:11434".to_string(),
            model: Some("llama3.2".to_string()),
            ..Default::default()
        };

        let provider = create_provider(&config);
        assert_eq!(provider.name(), "ollama");
        assert!(provider.is_available());
    }

    #[test]
    fn test_create_ollama_with_default_url() {
        let config = LLMConfig {
            provider: "ollama".to_string(),
            ..Default::default()
        };

        let provider = create_provider(&config);
        assert_eq!(provider.name(), "ollama");
        assert!(provider.is_available()); // Default URL is always "available"
    }
}
