use super::{LLMProvider, SessionContext, SummarizationResult};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

const OPENAI_API_URL: &str = "https://api.openai.com/v1/chat/completions";

/// OpenAI GPT provider for LLM summarization.
pub struct OpenAIProvider {
    client: Client,
    api_key: String,
    model: String,
    max_tokens: u32,
}

impl OpenAIProvider {
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
struct OpenAIRequest {
    model: String,
    max_tokens: u32,
    messages: Vec<Message>,
    response_format: ResponseFormat,
}

#[derive(Serialize)]
struct Message {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ResponseFormat {
    #[serde(rename = "type")]
    format_type: String,
}

#[derive(Deserialize)]
struct OpenAIResponse {
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Deserialize)]
struct Choice {
    message: ResponseMessage,
}

#[derive(Deserialize)]
struct ResponseMessage {
    content: Option<String>,
}

#[derive(Deserialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
}

#[derive(Deserialize)]
struct SummaryJson {
    summary: String,
    #[serde(rename = "type")]
    entry_type: Option<String>,
    key_files: Option<Vec<String>>,
}

#[async_trait]
impl LLMProvider for OpenAIProvider {
    async fn summarize(&self, context: &SessionContext) -> Result<SummarizationResult> {
        let prompt = self.build_prompt(context);

        let request = OpenAIRequest {
            model: self.model.clone(),
            max_tokens: self.max_tokens,
            messages: vec![Message {
                role: "user".to_string(),
                content: prompt,
            }],
            response_format: ResponseFormat {
                format_type: "json_object".to_string(),
            },
        };

        let response = self
            .client
            .post(OPENAI_API_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .context("failed to send request to OpenAI API")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(anyhow!("OpenAI API error ({}): {}", status, error_text));
        }

        let api_response: OpenAIResponse = response
            .json()
            .await
            .context("failed to parse OpenAI API response")?;

        let text = api_response
            .choices
            .first()
            .and_then(|c| c.message.content.as_ref())
            .ok_or_else(|| anyhow!("no content in OpenAI response"))?;

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
            .map(|u| u.prompt_tokens + u.completion_tokens);

        Ok(SummarizationResult {
            summary,
            suggested_type,
            key_files,
            tokens_used,
        })
    }

    fn name(&self) -> &'static str {
        "openai"
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
        let provider = OpenAIProvider::new("test-key".to_string(), "gpt-4o-mini".to_string(), 1024);

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
        let provider = OpenAIProvider::new("test-key".to_string(), "gpt-4o-mini".to_string(), 1024);

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
        let provider = OpenAIProvider::new("test-key".to_string(), "gpt-4o-mini".to_string(), 1024);

        // Create a diff larger than 4000 chars
        let large_diff = "a".repeat(5000);
        let context = SessionContext::new("test").with_git_diff(large_diff);

        let prompt = provider.build_prompt(&context);

        assert!(prompt.contains("(truncated)"));
        assert!(prompt.len() < 6000); // Should be truncated
    }

    #[test]
    fn test_is_available() {
        let provider =
            OpenAIProvider::new("sk-test-key".to_string(), "gpt-4o-mini".to_string(), 1024);
        assert!(provider.is_available());

        let empty_provider = OpenAIProvider::new(String::new(), "gpt-4o-mini".to_string(), 1024);
        assert!(!empty_provider.is_available());
    }
}
