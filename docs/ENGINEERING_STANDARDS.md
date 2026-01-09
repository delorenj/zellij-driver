# Engineering Standards & Anti-Patterns

This document defines the engineering standards, architectural guardrails, and anti-patterns for the `znav` (Zellij Driver) project. These standards are strictly enforced to ensure security, reliability, and maintainability.

## Architectural Anti-Patterns

### 1. "Wired-But-Not-Connected"
**Definition:** Implementing complex logic (like security filters, circuit breakers, or error handlers) that is structurally present but not invoked in the execution hot path.
**Impact:** Creates a false sense of security and reliability. Catastrophic credential leakage or cascading failures can occur despite "having code for that."

*   **❌ Wrong:** Defining a `SecretFilter` struct but constructing raw prompt strings.
*   **✅ Correct:** Using a `LazyLock<SecretFilter>` singleton and invoking `.filter_lines()` on *every* input source before it touches an LLM prompt. Proof of integration (metrics/logs) is required.

### 2. Global Circuit Breakers
**Definition:** Using a single, global circuit breaker instance for multiple distinct external dependencies.
**Impact:** A failure in one provider (e.g., Anthropic API outage) blocks all other providers (e.g., OpenAI, Local), causing total system failure.

*   **❌ Wrong:** `static BREAKER: Lazy<CircuitBreaker>` shared across all requests.
*   **✅ Correct:** `ProviderRegistry` pattern where each named provider has its own isolated `CircuitBreaker` instance.

### 3. Synchronous I/O in Async Contexts
**Definition:** Using blocking `std::fs` or `std::thread::sleep` calls within `async` functions running on the Tokio runtime.
**Impact:** Blocks the OS thread, preventing the runtime from scheduling other tasks, destroying concurrency and causing unpredictable latency spikes.

*   **❌ Wrong:** `std::fs::read_dir(path)?` inside an `async fn`.
*   **✅ Correct:** `tokio::fs::read_dir(path).await?`.

### 4. Monolithic Dispatchers
**Definition:** A single `main` function or massive `match` statement handling all CLI command logic.
**Impact:** Violates Single Responsibility Principle, makes testing impossible, and creates merge conflicts.

*   **❌ Wrong:** A 200-line `match` statement in `main.rs`.
*   **✅ Correct:** `Command` pattern with dedicated `Handler` structs (e.g., `PaneHandler`, `HistoryHandler`) implementing a common trait.

### 5. Silent Failures
**Definition:** Catching errors and printing them to stderr or ignoring them without structured logging.
**Impact:** Impossible to debug production issues or alert on degrading health.

*   **❌ Wrong:** `eprintln!("Error: {}", e);`
*   **✅ Correct:** `tracing::error!(error = %e, "Failed to execute operation");`

---

## Reliability Standards

### Timeouts
*   **Requirement:** All external calls (Network, Zellij CLI, Redis) **MUST** have explicit timeouts.
*   **Defaults:**
    *   Zellij commands: 5s
    *   Redis connection: 10s
    *   HTTP/LLM calls: 30s (configurable)
    *   RabbitMQ publish: 10s

### Circuit Breakers
*   **Requirement:** Every integration point with a 3rd party service (LLM APIs) must be wrapped in a circuit breaker.
*   **Metric:** Track `circuit_breaker.open` and `circuit_breaker.half_open` events.

### Saga Pattern
*   **Requirement:** Multi-step operations that modify state (e.g., "Write intent to Redis" + "Create Pane in Zellij") must use the Saga pattern.
*   **Mechanism:** If the second step fails, a compensating transaction (rollback) must be executed to revert the first step.

---

## Security Standards

### Secret Filtering
*   **Requirement:** **Zero Trust** for shell history and git diffs.
*   **Implementation:** All text leaving the local machine (to an LLM) must pass through the `SecretFilter`.
*   **Audit:** Log the *count* of redacted secrets, never the secrets themselves.

---

## Observability Standards

### Metrics
*   **Requirement:** "Metrics Prove It". If a feature is integrated, it must emit a metric.
*   **Critical Metrics:**
    *   `llm.latency_ms` (Histogram)
    *   `secrets.redacted_count` (Counter)
    *   `redis.operation_duration_ms` (Histogram)

### Structured Logging
*   **Library:** `tracing` crate.
*   **Format:** JSON in production, pretty-print in dev.
*   **Context:** Spans must capture relevant context (e.g., `pane_id`, `session_id`) at the entry point.
