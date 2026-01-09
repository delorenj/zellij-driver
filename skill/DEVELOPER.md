# Developer Guidelines & Skills

This document outlines the mandatory skills, directives, and processes for all agents and developers working on the `znav` (Zellij Driver) codebase.

## Imperative Directives (The "Definition of Done")

Any PR or code change must adhere to these rules before it is considered complete.

1.  **P0: Security First**
    *   **Mandatory Secret Filter:** Before *any* data is sent to an LLM provider, it must be processed by `SecretFilter::filter_lines()`.
    *   **Implementation:** Use a `LazyLock<SecretFilter>` singleton for performance.
    *   **Verification:** Integration tests must prove that secrets (e.g., `AWS_KEY=...`) are redacted in the generated prompt.

2.  **P0: Resilience & Reliability**
    *   **Timeouts:** Every external call (HTTP, Redis, Zellij CLI) must have a configured timeout.
    *   **Circuit Breakers:** All LLM provider calls must be wrapped in a circuit breaker. Use a `ProviderRegistry` to ensure one provider's failure doesn't block others.
    *   **No Panics:** Avoid `unwrap()` in production code. Use `anyhow::Result` propagation.

3.  **P0: Observability**
    *   **Metrics Prove It:** If you can't measure it, it doesn't exist. Add `metrics::counter!` or `metrics::histogram!` for every new feature path.
    *   **Structured Logging:** Use `tracing::info!`, `warn!`, `error!` with key-value pairs. Never use `println!` or `eprintln!` for application logging.

4.  **P0: Async Correctness**
    *   **No Blocking I/O:** Never use `std::fs` or `std::thread` inside async functions. Use `tokio::fs` and `tokio::time`.
    *   **Connection Pooling:** Use `bb8` or similar for Redis connections. Never create a new connection per request.

5.  **Naming & Consistency**
    *   **Binary Name:** `znav` (The project is zellij-driver, the binary is `znav`).
    *   **Redis Keys:** `znav:*` (or `perth:*` if migrating, but stick to the config default).
    *   **File Naming:** `snake_case` for files (e.g., `circuit_breaker.rs`).

---

## Knowledge Evolution Mechanism

As we build and refactor, we learn. We must capture this knowledge to prevent regression and educate future agents.

**When you learn a new pattern or fix a recurring architectural issue, record it in `docs/architecture-decisions.md` or similar, following this format:**

```markdown
### [Date] Pattern: [Name]

- **[Old Pattern]**: Describe the anti-pattern or previous approach.
  *Example: Global circuit breaker blocking all providers.*

- **[New Pattern]**: Describe the improved approach.
  *Example: Per-provider `ProviderRegistry` with isolated breakers.*

- **[Context/Ticket]**: Why was this changed?
  *Example: REL-001 - Anthropic outage caused system-wide lockup.*
```

### Critical Metrics to Track

When adding features, ensure these metrics are implemented:

- `secrets.redacted.total` (Counter)
- `circuit_breaker.{provider}.open` (Counter)
- `llm.summarize.duration_seconds` (Histogram)
- `bloodbank.published` (Counter)
- `batch_panes.compensation_actions` (Counter)
