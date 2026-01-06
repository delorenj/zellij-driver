use anyhow::{Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};

/// Default patterns for secret detection.
const DEFAULT_PATTERNS: &[&str] = &[
    // API keys and tokens
    r"(?i)(api[_-]?key|apikey)\s*[=:]\s*\S+",
    r"(?i)(secret[_-]?key|secretkey)\s*[=:]\s*\S+",
    r"(?i)(access[_-]?token|accesstoken)\s*[=:]\s*\S+",
    r"(?i)(auth[_-]?token|authtoken)\s*[=:]\s*\S+",
    r"(?i)bearer\s+[a-zA-Z0-9._-]+",
    // Passwords
    r"(?i)(password|passwd|pwd)\s*[=:]\s*\S+",
    // AWS
    r"(?i)aws[_-]?(access[_-]?key[_-]?id|secret[_-]?access[_-]?key)\s*[=:]\s*\S+",
    r"AKIA[0-9A-Z]{16}",  // AWS Access Key ID
    // GitHub/GitLab tokens
    r"gh[pousr]_[A-Za-z0-9_]{36,}",  // GitHub tokens
    r"glpat-[A-Za-z0-9_-]{20,}",  // GitLab PAT
    // Generic secrets
    r"(?i)(private[_-]?key|privatekey)\s*[=:]\s*\S+",
    r"(?i)(client[_-]?secret|clientsecret)\s*[=:]\s*\S+",
    // Database URLs with credentials
    r"(?i)(postgres|mysql|mongodb|redis)://[^:]+:[^@]+@",
    // SSH keys
    r"-----BEGIN\s+(RSA|DSA|EC|OPENSSH)\s+PRIVATE\s+KEY-----",
    // Generic env var patterns
    r"(?i)export\s+\w*(key|token|secret|password|credential)\w*\s*=\s*\S+",
];

/// Configuration for secret filtering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterConfig {
    /// Additional patterns to filter (regex)
    #[serde(default)]
    pub additional_patterns: Vec<String>,

    /// Patterns to exclude from filtering
    #[serde(default)]
    pub exclude_patterns: Vec<String>,

    /// Replacement text for redacted secrets
    #[serde(default = "default_replacement")]
    pub replacement: String,
}

fn default_replacement() -> String {
    "[REDACTED]".to_string()
}

impl Default for FilterConfig {
    fn default() -> Self {
        Self {
            additional_patterns: Vec::new(),
            exclude_patterns: Vec::new(),
            replacement: default_replacement(),
        }
    }
}

/// Secret filter for sanitizing text before LLM submission.
pub struct SecretFilter {
    patterns: Vec<Regex>,
    replacement: String,
}

impl SecretFilter {
    /// Create a new filter with default patterns.
    pub fn new() -> Result<Self> {
        Self::with_config(&FilterConfig::default())
    }

    /// Create a filter with custom configuration.
    pub fn with_config(config: &FilterConfig) -> Result<Self> {
        let mut patterns = Vec::new();

        // Compile default patterns
        for pattern in DEFAULT_PATTERNS {
            let regex = Regex::new(pattern)
                .with_context(|| format!("failed to compile default pattern: {}", pattern))?;
            patterns.push(regex);
        }

        // Add custom patterns
        for pattern in &config.additional_patterns {
            let regex = Regex::new(pattern)
                .with_context(|| format!("failed to compile custom pattern: {}", pattern))?;
            patterns.push(regex);
        }

        Ok(Self {
            patterns,
            replacement: config.replacement.clone(),
        })
    }

    /// Filter secrets from the given text.
    /// Returns the sanitized text and count of redactions made.
    pub fn filter(&self, text: &str) -> FilterResult {
        let mut result = text.to_string();
        let mut redaction_count = 0;

        for pattern in &self.patterns {
            let matches: Vec<_> = pattern.find_iter(&result).collect();
            redaction_count += matches.len();

            result = pattern.replace_all(&result, &self.replacement).to_string();
        }

        FilterResult {
            text: result,
            redaction_count,
        }
    }

    /// Filter multiple lines and return results.
    pub fn filter_lines(&self, lines: &[String]) -> (Vec<String>, usize) {
        let mut total_redactions = 0;
        let filtered: Vec<String> = lines
            .iter()
            .map(|line| {
                let result = self.filter(line);
                total_redactions += result.redaction_count;
                result.text
            })
            .collect();

        (filtered, total_redactions)
    }
}

impl Default for SecretFilter {
    fn default() -> Self {
        Self::new().expect("default patterns should compile")
    }
}

/// Result of filtering operation.
#[derive(Debug)]
pub struct FilterResult {
    /// The sanitized text
    pub text: String,

    /// Number of redactions made
    pub redaction_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_filter_api_key() {
        let filter = SecretFilter::new().unwrap();

        let result = filter.filter("api_key=sk-1234567890abcdef");
        assert!(!result.text.contains("sk-1234567890"));
        assert!(result.text.contains("[REDACTED]"));
        assert_eq!(result.redaction_count, 1);
    }

    #[test]
    fn test_filter_password() {
        let filter = SecretFilter::new().unwrap();

        let result = filter.filter("password: mysecretpassword123");
        assert!(!result.text.contains("mysecretpassword123"));
        assert!(result.text.contains("[REDACTED]"));
    }

    #[test]
    fn test_filter_bearer_token() {
        let filter = SecretFilter::new().unwrap();

        let result = filter.filter("Authorization: Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9");
        assert!(!result.text.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
    }

    #[test]
    fn test_filter_aws_key() {
        let filter = SecretFilter::new().unwrap();

        let result = filter.filter("AWS_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE");
        assert!(!result.text.contains("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn test_filter_github_token() {
        let filter = SecretFilter::new().unwrap();

        let result = filter.filter("ghp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx");
        assert!(!result.text.contains("ghp_"));
        assert!(result.text.contains("[REDACTED]"));
    }

    #[test]
    fn test_filter_database_url() {
        let filter = SecretFilter::new().unwrap();

        let result = filter.filter("postgres://user:secretpass@localhost:5432/db");
        assert!(!result.text.contains("secretpass"));
    }

    #[test]
    fn test_filter_private_key() {
        let filter = SecretFilter::new().unwrap();

        let result = filter.filter("-----BEGIN RSA PRIVATE KEY-----\nMIIEpAIBAAKCAQ...");
        assert!(result.text.contains("[REDACTED]"));
    }

    #[test]
    fn test_no_redaction_for_safe_text() {
        let filter = SecretFilter::new().unwrap();

        let safe_text = "cargo build --release\ngit status\nls -la";
        let result = filter.filter(safe_text);
        assert_eq!(result.text, safe_text);
        assert_eq!(result.redaction_count, 0);
    }

    #[test]
    fn test_filter_lines() {
        let filter = SecretFilter::new().unwrap();

        let lines = vec![
            "export API_KEY=secret123".to_string(),
            "cargo build".to_string(),
            "password: hunter2".to_string(),
        ];

        let (filtered, count) = filter.filter_lines(&lines);
        assert_eq!(filtered.len(), 3);
        assert!(count >= 2);
        assert!(!filtered[0].contains("secret123"));
        assert_eq!(filtered[1], "cargo build");
    }

    #[test]
    fn test_custom_pattern() {
        let config = FilterConfig {
            additional_patterns: vec![r"my_custom_secret_\d+".to_string()],
            ..Default::default()
        };

        let filter = SecretFilter::with_config(&config).unwrap();
        let result = filter.filter("found my_custom_secret_12345 here");
        assert!(!result.text.contains("my_custom_secret_12345"));
    }

    #[test]
    fn test_custom_replacement() {
        let config = FilterConfig {
            replacement: "***".to_string(),
            ..Default::default()
        };

        let filter = SecretFilter::with_config(&config).unwrap();
        let result = filter.filter("api_key=secret");
        assert!(result.text.contains("***"));
        assert!(!result.text.contains("[REDACTED]"));
    }
}
