use super::{LLMProvider, SessionContext, SummarizationResult};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const ANTHROPIC_API_URL: &str = "https://api.anthropic.com/v1/messages";
const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic Claude provider for LLM summarization.
pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
    max_tokens: u32,
}

impl AnthropicProvider {
    pub fn new(api_key: String, model: String, max_tokens: u32) -> Self {
        Self {
            client: Client::new(),
            api_key,
            model,
            max_tokens,
        }
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
                // Truncate large diffs
                if diff.len() > 4000 {
                    prompt.push_str(&diff[..4000]);
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
        prompt.push_str("Respond in this exact JSON format:\n");
        prompt.push_str(r#"{"summary": "...", "type": "checkpoint|milestone|exploration", "key_files": ["file1.rs", "file2.rs"]}"#);

        prompt
    }
}

#[derive(Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Deserialize)]
struct AnthropicResponse {
    content: Vec<ContentBlock>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    input_tokens: u32,
    output_tokens: u32,
}

#[derive(Deserialize)]
struct SummaryJson {
    summary: String,
    #[serde(rename = "type")]
    entry_type: Option<String>,
    key_files: Option<Vec<String>>,
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    async fn summarize(&self, context: &SessionContext) -> Result<SummarizationResult> {
        let prompt = self.build_prompt(context);

        let request = AnthropicRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt,
            }],
        };

        let response = self
            .client
            .post(ANTHROPIC_API_URL)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await
            .context("failed to send request to Anthropic API")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Anthropic API error ({}): {}",
                status,
                error_text
            ));
        }

        let api_response: AnthropicResponse = response
            .json()
            .await
            .context("failed to parse Anthropic API response")?;

        let text = api_response
            .content
            .first()
            .and_then(|c| c.text.as_ref())
            .ok_or_else(|| anyhow!("no text content in Anthropic response"))?;

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

        let tokens_used = api_response
            .usage
            .map(|u| u.input_tokens + u.output_tokens);

        Ok(SummarizationResult {
            summary,
            suggested_type,
            key_files,
            tokens_used,
        })
    }

    fn name(&self) -> &'static str {
        "anthropic"
    }

    fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_prompt_basic() {
        let provider = AnthropicProvider::new(
            "test-key".to_string(),
            "claude-sonnet-4-20250514".to_string(),
            1024,
        );

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
        let provider = AnthropicProvider::new(
            "test-key".to_string(),
            "claude-sonnet-4-20250514".to_string(),
            1024,
        );

        let context = SessionContext::new("build")
            .with_shell_history(vec![
                "cargo build".to_string(),
                "cargo test".to_string(),
            ]);

        let prompt = provider.build_prompt(&context);

        assert!(prompt.contains("cargo build"));
        assert!(prompt.contains("cargo test"));
    }

    #[test]
    fn test_is_available() {
        let provider = AnthropicProvider::new(
            "sk-test-key".to_string(),
            "claude-sonnet-4-20250514".to_string(),
            1024,
        );
        assert!(provider.is_available());

        let empty_provider = AnthropicProvider::new(
            String::new(),
            "claude-sonnet-4-20250514".to_string(),
            1024,
        );
        assert!(!empty_provider.is_available());
    }
}
