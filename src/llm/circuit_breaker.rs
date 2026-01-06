//! Circuit breaker for LLM API calls.
//!
//! Prevents cascading failures by tracking consecutive errors and temporarily
//! blocking calls when the failure threshold is reached.

use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Circuit breaker states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CircuitState {
    /// Normal operation - requests are allowed
    Closed,
    /// Too many failures - requests blocked
    Open,
    /// Cooling down - allowing test requests
    HalfOpen,
}

/// Configuration for the circuit breaker
#[derive(Debug, Clone)]
pub struct CircuitBreakerConfig {
    /// Number of consecutive failures before opening the circuit
    pub failure_threshold: u32,
    /// How long to wait before trying again (half-open state)
    pub cooldown_duration: Duration,
}

impl Default for CircuitBreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            cooldown_duration: Duration::from_secs(5 * 60), // 5 minutes
        }
    }
}

/// Thread-safe circuit breaker for LLM API calls.
///
/// Uses atomic operations for lock-free, thread-safe state management.
/// State is stored in memory and resets when the process restarts.
pub struct CircuitBreaker {
    /// Number of consecutive failures
    consecutive_failures: AtomicU32,
    /// Timestamp (epoch millis) when circuit was opened
    opened_at: AtomicU64,
    /// Configuration
    config: CircuitBreakerConfig,
}

impl CircuitBreaker {
    /// Create a new circuit breaker with default configuration.
    pub fn new() -> Self {
        Self::with_config(CircuitBreakerConfig::default())
    }

    /// Create a new circuit breaker with custom configuration.
    pub fn with_config(config: CircuitBreakerConfig) -> Self {
        Self {
            consecutive_failures: AtomicU32::new(0),
            opened_at: AtomicU64::new(0),
            config,
        }
    }

    /// Get the current state of the circuit breaker.
    pub fn state(&self) -> CircuitState {
        let failures = self.consecutive_failures.load(Ordering::Acquire);

        if failures < self.config.failure_threshold {
            return CircuitState::Closed;
        }

        // Circuit has been opened - check if cooldown has elapsed
        let opened_at_millis = self.opened_at.load(Ordering::Acquire);
        if opened_at_millis == 0 {
            return CircuitState::Open;
        }

        let now_millis = current_epoch_millis();
        let elapsed_millis = now_millis.saturating_sub(opened_at_millis);
        let cooldown_millis = self.config.cooldown_duration.as_millis() as u64;

        if elapsed_millis >= cooldown_millis {
            CircuitState::HalfOpen
        } else {
            CircuitState::Open
        }
    }

    /// Check if a request should be allowed.
    ///
    /// Returns `Ok(())` if the request can proceed, or `Err` with a message
    /// explaining why the circuit is open.
    pub fn allow_request(&self) -> Result<(), String> {
        match self.state() {
            CircuitState::Closed => Ok(()),
            CircuitState::HalfOpen => Ok(()), // Allow test request
            CircuitState::Open => {
                let opened_at_millis = self.opened_at.load(Ordering::Acquire);
                let now_millis = current_epoch_millis();
                let elapsed = Duration::from_millis(now_millis.saturating_sub(opened_at_millis));
                let remaining = self.config.cooldown_duration.saturating_sub(elapsed);

                Err(format!(
                    "LLM circuit breaker is open due to {} consecutive failures.\n\
                    Will retry in {} seconds.\n\n\
                    You can still log entries manually:\n\
                    zdrive pane log <PANE> <SUMMARY>",
                    self.consecutive_failures.load(Ordering::Acquire),
                    remaining.as_secs()
                ))
            }
        }
    }

    /// Record a successful request.
    ///
    /// Resets the failure counter and closes the circuit.
    pub fn record_success(&self) {
        self.consecutive_failures.store(0, Ordering::Release);
        self.opened_at.store(0, Ordering::Release);
    }

    /// Record a failed request.
    ///
    /// Increments the failure counter and opens the circuit if threshold is reached.
    pub fn record_failure(&self) {
        let previous = self.consecutive_failures.fetch_add(1, Ordering::AcqRel);
        let new_count = previous + 1;

        // If we just hit the threshold, record when the circuit opened
        if new_count == self.config.failure_threshold {
            let now_millis = current_epoch_millis();
            self.opened_at.store(now_millis, Ordering::Release);
        }
    }

    /// Get the number of consecutive failures.
    pub fn failure_count(&self) -> u32 {
        self.consecutive_failures.load(Ordering::Acquire)
    }

    /// Reset the circuit breaker to closed state.
    #[cfg(test)]
    pub fn reset(&self) {
        self.consecutive_failures.store(0, Ordering::Release);
        self.opened_at.store(0, Ordering::Release);
    }
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self::new()
    }
}

/// Get current time as milliseconds since Unix epoch.
fn current_epoch_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_initial_state_is_closed() {
        let cb = CircuitBreaker::new();
        assert_eq!(cb.state(), CircuitState::Closed);
        assert!(cb.allow_request().is_ok());
    }

    #[test]
    fn test_stays_closed_under_threshold() {
        let cb = CircuitBreaker::new();

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Closed);

        // Still under threshold (3)
        assert!(cb.allow_request().is_ok());
    }

    #[test]
    fn test_opens_at_threshold() {
        let cb = CircuitBreaker::new();

        cb.record_failure();
        cb.record_failure();
        cb.record_failure(); // 3rd failure

        assert_eq!(cb.state(), CircuitState::Open);
        assert!(cb.allow_request().is_err());
    }

    #[test]
    fn test_success_resets_failures() {
        let cb = CircuitBreaker::new();

        cb.record_failure();
        cb.record_failure();
        cb.record_success();

        assert_eq!(cb.failure_count(), 0);
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_success_closes_circuit() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            cooldown_duration: Duration::from_millis(1), // Very short for testing
        };
        let cb = CircuitBreaker::with_config(config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();

        // Wait for cooldown
        std::thread::sleep(Duration::from_millis(10));

        // Should be half-open now
        assert_eq!(cb.state(), CircuitState::HalfOpen);

        // Success should close it
        cb.record_success();
        assert_eq!(cb.state(), CircuitState::Closed);
    }

    #[test]
    fn test_half_open_allows_request() {
        let config = CircuitBreakerConfig {
            failure_threshold: 3,
            cooldown_duration: Duration::from_millis(1),
        };
        let cb = CircuitBreaker::with_config(config);

        // Open the circuit
        cb.record_failure();
        cb.record_failure();
        cb.record_failure();

        // Wait for cooldown
        std::thread::sleep(Duration::from_millis(10));

        // Should allow test request in half-open state
        assert_eq!(cb.state(), CircuitState::HalfOpen);
        assert!(cb.allow_request().is_ok());
    }

    #[test]
    fn test_error_message_suggests_manual_logging() {
        let cb = CircuitBreaker::new();

        cb.record_failure();
        cb.record_failure();
        cb.record_failure();

        let err = cb.allow_request().unwrap_err();
        assert!(err.contains("zdrive pane log"));
        assert!(err.contains("consecutive failures"));
    }

    #[test]
    fn test_custom_threshold() {
        let config = CircuitBreakerConfig {
            failure_threshold: 5,
            cooldown_duration: Duration::from_secs(60),
        };
        let cb = CircuitBreaker::with_config(config);

        // 4 failures should still be closed
        for _ in 0..4 {
            cb.record_failure();
        }
        assert_eq!(cb.state(), CircuitState::Closed);

        // 5th failure opens it
        cb.record_failure();
        assert_eq!(cb.state(), CircuitState::Open);
    }
}
