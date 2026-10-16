# Operator Runbook

This runbook describes operational procedures for running and maintaining the escrow funding system.

## Funding Rate Limiting

The funding rate limiter protects the protocol from abnormal funding activity by capping the amount of funding that can be processed within a rolling window. When the configured limit is exceeded, funding is automatically paused until an operator resumes it.

### `set_max_funding_rate`

Configures the maximum funding rate allowed before the limiter trips.

- **What it configures:** the maximum amount of funding (in the funding asset's base units) that may be processed per rolling window.
- **How the limit is measured:** the limiter tracks cumulative funding volume over a rolling time window. Each funding operation adds to the running total; once the total for the current window meets or exceeds the configured maximum, the limiter trips and funding is paused automatically.
- **Example:**

  ```
  set_max_funding_rate(1_000_000)
  ```

  With a maximum of `1_000_000`, funding operations are allowed until cumulative volume in the window reaches `1_000_000`. The next funding attempt that would exceed the limit trips the limiter.

### `is_funding_paused`

Reports whether funding is currently paused.

- **How to check pause state:** call `is_funding_paused()`. It returns `true` when funding is paused and `false` when funding is active.
- **What triggers auto-pause:** the limiter pauses funding automatically when the cumulative funding volume in the rolling window reaches the configured maximum set by `set_max_funding_rate`. No operator action is required for the pause itself.
- **Example:**

  ```
  if is_funding_paused() {
      // funding is halted; investigate volume before resuming
  }
  ```

### `resume_funding`

Resumes funding after it has been paused.

- **When to use:** after an automatic pause caused by the rate limiter, once the operator has confirmed the activity is expected and safe to continue.
- **Effect:** clears the paused state so funding operations can proceed again. The rolling window accounting continues to apply, so a resumed system will pause again if the limit is reached.
- **Example:**

  ```
  resume_funding()
  ```

### Operational guidance

1. Monitor `is_funding_paused()` as part of routine health checks.
2. When a pause occurs, review recent funding volume to confirm it is legitimate.
3. If the activity is expected, call `resume_funding()` to restore service.
4. If the activity is unexpected, investigate before resuming and consider adjusting `set_max_funding_rate` to a value appropriate for normal operating conditions.
