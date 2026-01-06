use super::{LLMProvider, SessionContext, SummarizationResult};
use anyhow::{anyhow, Result};
use async_trait::async_trait;

/// A no-op provider that returns an error when called.
/// Used when LLM is disabled or misconfigured.
pub struct NoOpProvider {
    reason: String,
}

impl NoOpProvider {
    pub fn new(reason: impl Into<String>) -> Self {
        Self {
            reason: reason.into(),
        }
    }
}

#[async_trait]
impl LLMProvider for NoOpProvider {
    async fn summarize(&self, _context: &SessionContext) -> Result<SummarizationResult> {
        Err(anyhow!("LLM unavailable: {}", self.reason))
    }

    fn name(&self) -> &'static str {
        "noop"
    }

    fn is_available(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_noop_returns_error() {
        let provider = NoOpProvider::new("test reason");
        let ctx = SessionContext::new("test");

        let result = provider.summarize(&ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("test reason"));
    }

    #[test]
    fn test_noop_not_available() {
        let provider = NoOpProvider::new("disabled");
        assert!(!provider.is_available());
        assert_eq!(provider.name(), "noop");
    }
}
