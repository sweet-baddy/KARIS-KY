//! REPL developer-experience helpers: per-entrypoint help, tab completion,
//! `--json` output, and retry for transient RPC failures.
//!
//! Entrypoint metadata is sourced from `sdk-ts/spec.json`, embedded at
//! compile time so the REPL always matches the published contract spec.

// The retry helpers are not yet called outside tests: live Soroban RPC
// invocation is a planned enhancement (see README "Future Enhancements").
#![cfg_attr(not(test), allow(dead_code))]

use serde_json::{json, Value};
use std::thread::sleep;
use std::time::Duration;

/// Contract spec embedded at compile time.
const SPEC_JSON: &str = include_str!("../../sdk-ts/spec.json");

/// Returns the parsed contract spec.
fn spec() -> Value {
    serde_json::from_str(SPEC_JSON).expect("sdk-ts/spec.json must be valid JSON")
}

/// Returns all entrypoint objects from the spec.
fn entrypoints() -> Vec<Value> {
    spec()["entrypoints"]
        .as_array()
        .cloned()
        .unwrap_or_default()
}

/// Returns every entrypoint name declared in the contract spec.
pub fn entrypoint_names() -> Vec<String> {
    entrypoints()
        .iter()
        .filter_map(|e| e["name"].as_str().map(str::to_string))
        .collect()
}

/// Returns entrypoint names starting with `prefix`, sorted, for tab completion.
pub fn complete(prefix: &str) -> Vec<String> {
    let mut matches: Vec<String> = entrypoint_names()
        .into_iter()
        .filter(|n| n.starts_with(prefix))
        .collect();
    matches.sort();
    matches
}

/// Renders help (description, auth, args, return type) for an entrypoint.
///
/// Returns `None` if `name` is not an entrypoint in the spec.
pub fn help_text(name: &str) -> Option<String> {
    let ep = entrypoints().into_iter().find(|e| e["name"] == name)?;
    let mut out = format!("{name}\n  {}\n", ep["description"].as_str().unwrap_or(""));
    if let Some(auth) = ep["auth"].as_str() {
        out.push_str(&format!("  auth: {auth}\n"));
    }
    out.push_str("  args:\n");
    let params = ep["params"].as_array().cloned().unwrap_or_default();
    if params.is_empty() {
        out.push_str("    (none)\n");
    }
    for p in params {
        out.push_str(&format!(
            "    {}: {} - {}\n",
            p["name"].as_str().unwrap_or("?"),
            p["type"].as_str().unwrap_or("?"),
            p["description"].as_str().unwrap_or("")
        ));
    }
    let ret = &ep["returns"];
    match ret["type"].as_str() {
        Some(ty) => out.push_str(&format!(
            "  returns: {ty} - {}\n",
            ret["description"].as_str().unwrap_or("")
        )),
        None => out.push_str("  returns: ()\n"),
    }
    Some(out)
}

/// Builds the `--json` representation of a parsed command and its arguments.
pub fn command_json(command: &str, args: &[String]) -> Value {
    json!({
        "command": command,
        "args": args,
        "known_entrypoint": entrypoint_names().iter().any(|n| n == command),
    })
}

/// Classification of an RPC failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcError {
    /// Temporary failure (timeout, connection reset, 5xx); safe to retry reads.
    Transient(String),
    /// Permanent failure; never retried.
    Fatal(String),
}

/// Maximum attempts (initial call + retries) for read-only RPC calls.
pub const MAX_ATTEMPTS: u32 = 3;
/// Initial backoff; doubles after each failed attempt (100ms, 200ms, ...).
pub const BASE_DELAY: Duration = Duration::from_millis(100);

/// Runs a read-only RPC call, retrying [`RpcError::Transient`] failures up to
/// [`MAX_ATTEMPTS`] times with exponential backoff starting at `base_delay`.
///
/// [`RpcError::Fatal`] errors are returned immediately. Do not use this for
/// mutating calls: a retried write could be applied twice.
pub fn retry_read<T>(
    base_delay: Duration,
    mut op: impl FnMut() -> Result<T, RpcError>,
) -> Result<T, RpcError> {
    let mut delay = base_delay;
    let mut attempt = 1;
    loop {
        match op() {
            Err(RpcError::Transient(_)) if attempt < MAX_ATTEMPTS => {
                sleep(delay);
                delay *= 2;
                attempt += 1;
            }
            result => return result,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_entrypoint_has_help_text() {
        let names = entrypoint_names();
        assert!(!names.is_empty());
        for name in names {
            let help = help_text(&name).expect("help renders");
            assert!(help.starts_with(&name));
            assert!(help.contains("args:"));
            assert!(help.contains("returns:"));
        }
        assert!(help_text("not_an_entrypoint").is_none());
    }

    #[test]
    fn completion_suggests_matching_entrypoints() {
        let suggestions = complete("fund");
        assert!(suggestions.contains(&"fund".to_string()));
        assert!(suggestions.contains(&"fund_with_commitment".to_string()));
        assert!(suggestions.iter().all(|s| s.starts_with("fund")));
        assert!(complete("zzz_nope").is_empty());
    }

    #[test]
    fn json_output_is_valid_json() {
        for (cmd, args) in [
            ("status", vec![]),
            ("fund", vec!["GABC".to_string(), "100".to_string()]),
            ("get_escrow", vec![]),
        ] {
            let text = command_json(cmd, &args).to_string();
            let parsed: Value = serde_json::from_str(&text).expect("valid JSON");
            assert_eq!(parsed["command"], cmd);
        }
    }

    #[test]
    fn transient_errors_are_retried_until_success() {
        let mut calls = 0;
        let result = retry_read(Duration::ZERO, || {
            calls += 1;
            if calls < 3 {
                Err(RpcError::Transient("timeout".into()))
            } else {
                Ok(42)
            }
        });
        assert_eq!(result, Ok(42));
        assert_eq!(calls, 3);
    }

    #[test]
    fn fatal_errors_are_not_retried() {
        let mut calls = 0;
        let result: Result<(), _> = retry_read(Duration::ZERO, || {
            calls += 1;
            Err(RpcError::Fatal("bad request".into()))
        });
        assert_eq!(result, Err(RpcError::Fatal("bad request".into())));
        assert_eq!(calls, 1);
    }
}
