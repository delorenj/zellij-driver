use super::{LLMProvider, SessionContext, SummarizationResult};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Ollama provider for local LLM summarization.
pub struct OllamaProvider {
    client: Client,
    endpoint: String,
    model: String,
}

impl OllamaProvider {
    pub fn new(endpoint: String, model: String) -> Self {
        Self {
            client: Client::new(),
            endpoint,
            model,
        }
    }

    fn api_url(&self) -> String {
        format!("{}/api/generate", self.endpoint.trim_end_matches('/'))
    }

    fn build_prompt(&self, context: &SessionContext) -> String {
        let mut prompt = String::new();

        prompt.push_str("You are a developer assistant helping to summarize a coding session. ");
        prompt.push_str("Based on the following context, generate a concise summary of what was accomplished.\n\n");

        prompt.push_str(&format!("## Pane: {}\n\n", context.pane_name));

        if let Some(branch) = &context.git_branch {
            prompt.push_str(&format!("## Git Branch: {}\n\n", branch));
        }

        if !context.cwd.is_empty() {
            prompt.push_str(&format!("## Working Directory: {}\n\n", context.cwd));
        }

        if !context.shell_history.is_empty() {
            prompt.push_str("## Recent Commands:\n```\n");
            for cmd in &context.shell_history {
                prompt.push_str(cmd);
                prompt.push('\n');
            }
            prompt.push_str("```\n\n");
        }

        if let Some(diff) = &context.git_diff {
            if !diff.is_empty() {
                prompt.push_str("## Git Diff:\n```diff\n");
                // Truncate large diffs (Ollama has smaller context windows)
                if diff.len() > 2000 {
                    prompt.push_str(&diff[..2000]);
                    prompt.push_str("\n... (truncated)\n");
                } else {
                    prompt.push_str(diff);
                }
                prompt.push_str("```\n\n");
            }
        }

        if !context.active_files.is_empty() {
            prompt.push_str("## Active Files:\n");
            for file in &context.active_files {
                prompt.push_str(&format!("- {}\n", file));
            }
            prompt.push('\n');
        }

        if let Some(existing) = &context.existing_summary {
            prompt.push_str(&format!("## Previous Summary:\n{}\n\n", existing));
        }

        prompt.push_str("## Instructions:\n");
        prompt.push_str("1. Generate a brief (1-2 sentence) summary of what was accomplished\n");
        prompt.push_str("2. Suggest whether this is a 'milestone', 'checkpoint', or 'exploration'\n");
        prompt.push_str("3. List any key files that were modified\n\n");
        prompt.push_str("Respond in this exact JSON format (no markdown, just the JSON):\n");
        prompt.push_str(r#"{"summary": "...", "type": "checkpoint|milestone|exploration", "key_files": ["file1.rs", "file2.rs"]}"#);

        prompt
    }
}

#[derive(Serialize)]
struct OllamaRequest {
    model: String,
    prompt: String,
    stream: bool,
    format: String,
}

#[derive(Deserialize)]
struct OllamaResponse {
    response: String,
    #[serde(default)]
    eval_count: Option<u32>,
    #[serde(default)]
    prompt_eval_count: Option<u32>,
}

#[derive(Deserialize)]
struct SummaryJson {
    summary: String,
    #[serde(rename = "type")]
    entry_type: Option<String>,
    key_files: Option<Vec<String>>,
}

#[async_trait]
impl LLMProvider for OllamaProvider {
    async fn summarize(&self, context: &SessionContext) -> Result<SummarizationResult> {
        let prompt = self.build_prompt(context);

        let request = OllamaRequest {
            model: self.model.clone(),
            prompt,
            stream: false,
            format: "json".to_string(),
        };

        let response = self
            .client
            .post(&self.api_url())
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("failed to send request to Ollama API")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("Ollama API error ({}): {}", status, error_text));
        }

        let api_response: OllamaResponse = response
            .json()
            .await
            .context("failed to parse Ollama API response")?;

        let text = &api_response.response;

        // Try to parse as JSON, fall back to using raw text as summary
        let (summary, suggested_type, key_files) = match serde_json::from_str::<SummaryJson>(text) {
            Ok(parsed) => (
                parsed.summary,
                parsed.entry_type,
                parsed.key_files.unwrap_or_default(),
            ),
            Err(_) => {
                // If not valid JSON, use the raw text as the summary
                (text.clone(), None, Vec::new())
            }
        };

        // Ollama provides eval_count (output tokens) and prompt_eval_count (input tokens)
        let tokens_used = match (api_response.prompt_eval_count, api_response.eval_count) {
            (Some(input), Some(output)) => Some(input + output),
            (Some(input), None) => Some(input),
            (None, Some(output)) => Some(output),
            (None, None) => None,
        };

        Ok(SummarizationResult {
            summary,
            suggested_type,
            key_files,
            tokens_used,
        })
    }

    fn name(&self) -> &'static str {
        "ollama"
    }

    fn is_available(&self) -> bool {
        // Ollama is available if we have an endpoint configured
        !self.endpoint.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_basic() {
        let provider =
            OllamaProvider::new("http://localhost:11434".to_string(), "llama3.2".to_string());

        let context = SessionContext::new("test-pane")
            .with_cwd("/home/user/project")
            .with_git_branch("main");

        let prompt = provider.build_prompt(&context);

        assert!(prompt.contains("test-pane"));
        assert!(prompt.contains("/home/user/project"));
        assert!(prompt.contains("main"));
        assert!(prompt.contains("JSON format"));
    }

    #[test]
    fn test_build_prompt_with_commands() {
        let provider =
            OllamaProvider::new("http://localhost:11434".to_string(), "llama3.2".to_string());

        let context = SessionContext::new("build").with_shell_history(vec![
            "cargo build".to_string(),
            "cargo test".to_string(),
        ]);

        let prompt = provider.build_prompt(&context);

        assert!(prompt.contains("cargo build"));
        assert!(prompt.contains("cargo test"));
    }

    #[test]
    fn test_build_prompt_truncates_large_diff() {
        let provider =
            OllamaProvider::new("http://localhost:11434".to_string(), "llama3.2".to_string());

        // Create a diff larger than 2000 chars (smaller limit for Ollama)
        let large_diff = "a".repeat(3000);
        let context = SessionContext::new("test").with_git_diff(large_diff);

        let prompt = provider.build_prompt(&context);

        assert!(prompt.contains("(truncated)"));
        assert!(prompt.len() < 4000); // Should be truncated
    }

    #[test]
    fn test_api_url() {
        let provider =
            OllamaProvider::new("http://localhost:11434".to_string(), "llama3.2".to_string());
        assert_eq!(provider.api_url(), "http://localhost:11434/api/generate");

        // Test with trailing slash
        let provider2 =
            OllamaProvider::new("http://localhost:11434/".to_string(), "llama3.2".to_string());
        assert_eq!(provider2.api_url(), "http://localhost:11434/api/generate");
    }

    #[test]
    fn test_is_available() {
        let provider =
            OllamaProvider::new("http://localhost:11434".to_string(), "llama3.2".to_string());
        assert!(provider.is_available());

        let empty_provider = OllamaProvider::new(String::new(), "llama3.2".to_string());
        assert!(!empty_provider.is_available());
    }
}
