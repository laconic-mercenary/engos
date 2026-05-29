//! Anthropic API client — minimal wrapper over the Messages API.
//!
//! Phase 3 will expand this into the full `ModelBackend` implementation.
//! For now it provides a connectivity check used by the `--check` CLI flag
//! so operators can confirm their API key and model ID are valid before
//! running a real engagement.

/// Base URL for all Anthropic API calls.
pub const ENDPOINT: &str = "https://api.anthropic.com/v1/messages";

/// Anthropic API version header sent with every request.
/// Check <https://docs.anthropic.com/en/api/versioning> for updates.
pub const API_VERSION: &str = "2023-06-01";

// ── Connectivity check ────────────────────────────────────────────────────────

/// Send a minimal 1-token request to confirm the API key and model ID are valid.
///
/// Uses `reqwest::blocking` — this is called before the TUI starts (from
/// `--check` CLI mode) so there is no tokio runtime active.
///
/// # Returns
///
/// - `Ok(usage_hint)` — API accepted the request; `usage_hint` shows how
///   many input tokens the test consumed (tiny but non-zero).
/// - `Err(message)` — request failed or the API returned a non-2xx status.
///   The error string includes the HTTP status and the API error body so the
///   operator can diagnose key, model, or network issues.
pub fn check_connection(model_id: &str, api_key: &str) -> Result<String, String> {
    // Minimal body — 1 max_token means the model returns almost immediately
    // and costs essentially nothing, but the API still validates the key and
    // model ID fully before responding.
    let body = format!(
        r#"{{"model":"{model_id}","max_tokens":1,"messages":[{{"role":"user","content":"ping"}}]}}"#
    );

    let client = reqwest::blocking::Client::builder()
        // Fail quickly if the network or API is unreachable.
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .map_err(|e| format!("could not build HTTP client: {e}"))?;

    let response = client
        .post(ENDPOINT)
        .header("x-api-key",          api_key)
        .header("anthropic-version",   API_VERSION)
        .header("content-type",        "application/json")
        .body(body)
        .send()
        .map_err(|e| format!("network error: {e}"))?;

    let status = response.status();

    if status.is_success() {
        // Read the body to extract a usage hint — demonstrates the response
        // parses and gives the operator a concrete signal the call worked.
        let text = response.text().unwrap_or_default();
        let tokens = extract_input_tokens(&text);
        Ok(format!("used ~{tokens} input tokens"))
    } else {
        // Forward the full API error body so the operator can diagnose the
        // problem (wrong key, unknown model ID, rate limit, etc.).
        let text = response.text().unwrap_or_default();
        Err(format!("HTTP {status} — {}", compact_error(&text)))
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Derive the bare Anthropic model ID from an orchestrator display name.
///
/// Convention: display names are `"anthropic <model-id>"`, e.g.
/// `"anthropic claude-opus-4-7"` → `"claude-opus-4-7"`.
/// If the prefix is absent, the name is returned as-is so custom entries work.
pub fn model_id(orchestrator_name: &str) -> &str {
    orchestrator_name
        .strip_prefix("anthropic ")
        .unwrap_or(orchestrator_name)
}

/// Extract the `input_tokens` count from a raw JSON response body.
///
/// Returns `"?"` rather than failing if the field is absent or the JSON is
/// malformed — this is a best-effort display hint, not a hard requirement.
fn extract_input_tokens(body: &str) -> String {
    // Look for `"input_tokens":N` anywhere in the body without pulling in a
    // full JSON parser for this minor display field.
    if let Some(pos) = body.find("\"input_tokens\":") {
        let after = &body[pos + 16..];
        let end   = after.find(|c: char| !c.is_ascii_digit()).unwrap_or(after.len());
        let n     = &after[..end];
        if !n.is_empty() { return n.to_string(); }
    }
    "?".to_string()
}

/// Collapse a potentially long API error JSON body to the most useful part.
///
/// Anthropic errors look like `{"type":"error","error":{"type":"...","message":"..."}}`.
/// Extract just the message string when present; fall back to the raw body.
fn compact_error(body: &str) -> String {
    if let Some(pos) = body.find("\"message\":\"") {
        let after = &body[pos + 11..];
        if let Some(end) = after.find('"') {
            return after[..end].to_string();
        }
    }
    // Body is not the expected format — return it truncated.
    let truncated = &body[..body.len().min(200)];
    truncated.to_string()
}
