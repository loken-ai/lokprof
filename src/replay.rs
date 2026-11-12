//! Replay of a recorded session: requests sent at the offsets a trace holds.
//!
//! A trace is a file of JSON lines, one request per line.
//! The arrivals are fixed by the file, whatever the engine has answered, like the offered load.
//! The record names the trace by the SHA-256 of its bytes, so 2 engines replayed on the same
//! trace are known to have been offered the same requests.
//!
//! A line holds `offset_ms`, an optional `session`, a `prompt` or a list of `messages`
//! (`role` and `content`), and an optional `max_tokens`.
//! The requests of a session grow: the prompt sent is the parts of the session so far, joined
//! by an empty line.
//! A line of a session therefore holds the new parts only.
//!
//! `scripts/session_to_trace.py` writes a trace from the log of an agent session.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::future::Future;
use std::time::Duration;

/// Joins the parts of a session into one prompt.
const JOINER: &str = "\n\n";

/// One line of a trace.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Line {
    /// Milliseconds from the start of the session to the send.
    offset_ms: f64,
    /// Requests of one session share a prefix that grows.
    #[serde(default)]
    session: Option<String>,
    /// The text of the request. Exactly 1 of `prompt` and `messages` is given.
    #[serde(default)]
    prompt: Option<String>,
    #[serde(default)]
    messages: Option<Vec<Message>>,
    /// Defaults to the `--max-tokens` of the run.
    #[serde(default)]
    max_tokens: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Message {
    role: String,
    content: String,
}

/// A request as it is sent.
#[derive(Debug, Clone, PartialEq)]
pub struct Planned {
    /// The line of the trace, from 1.
    pub line: usize,
    pub offset_ms: f64,
    pub session: Option<String>,
    /// The whole prompt: the earlier parts of the session, then this part.
    pub prompt: String,
    pub max_tokens: Option<usize>,
}

/// A validated trace.
#[derive(Debug, Clone)]
pub struct Trace {
    /// SHA-256 of the bytes of the file, in hexadecimal.
    pub digest: String,
    pub requests: Vec<Planned>,
}

impl Trace {
    pub fn load(path: &str) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("cannot read the trace: {e}"))?;
        Self::parse(&bytes)
    }

    /// Reads a trace. A blank line is skipped.
    /// Any other line that is not valid is an error that names its number.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let text =
            std::str::from_utf8(bytes).map_err(|e| format!("the trace is not UTF-8: {e}"))?;
        let digest = format!("{:x}", Sha256::digest(bytes));
        let mut requests: Vec<Planned> = Vec::new();
        // The text sent so far, by session.
        let mut history: BTreeMap<String, String> = BTreeMap::new();
        for (i, raw) in text.lines().enumerate() {
            let n = i + 1;
            if raw.trim().is_empty() {
                continue;
            }
            let line: Line =
                serde_json::from_str(raw).map_err(|e| format!("trace line {n}: {e}"))?;
            if !line.offset_ms.is_finite() || line.offset_ms < 0.0 {
                return Err(format!("trace line {n}: offset_ms must be 0 or more"));
            }
            if let Some(last) = requests.last() {
                if line.offset_ms < last.offset_ms {
                    return Err(format!(
                        "trace line {n}: offset_ms goes back before line {}",
                        last.line
                    ));
                }
            }
            if line.max_tokens == Some(0) {
                return Err(format!("trace line {n}: max_tokens must be above 0"));
            }
            let part = match (&line.prompt, &line.messages) {
                (Some(p), None) if !p.is_empty() => p.clone(),
                (None, Some(m)) if !m.is_empty() => m
                    .iter()
                    .map(|m| format!("{}: {}", m.role, m.content))
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => {
                    return Err(format!(
                        "trace line {n}: give exactly 1 of a non-empty prompt and messages"
                    ))
                }
            };
            let prompt = match &line.session {
                Some(s) => {
                    let held = history.entry(s.clone()).or_default();
                    if !held.is_empty() {
                        held.push_str(JOINER);
                    }
                    held.push_str(&part);
                    held.clone()
                }
                None => part,
            };
            requests.push(Planned {
                line: n,
                offset_ms: line.offset_ms,
                session: line.session,
                prompt,
                max_tokens: line.max_tokens,
            });
        }
        if requests.is_empty() {
            return Err("the trace holds no request".to_string());
        }
        Ok(Self { digest, requests })
    }

    /// The sessions of the trace.
    pub fn sessions(&self) -> usize {
        self.requests
            .iter()
            .filter_map(|r| r.session.as_deref())
            .collect::<BTreeSet<_>>()
            .len()
    }

    /// What the record holds of the trace, in the settings of its cell.
    pub fn settings(&self) -> serde_json::Value {
        serde_json::json!({
            "trace_sha256": self.digest,
            "requests": self.requests.len(),
            "sessions": self.sessions(),
            "duration_ms": self.requests.last().map_or(0.0, |r| r.offset_ms),
        })
    }
}

/// Sends every request at its offset and returns what `send` gave, in trace order.
/// The clock starts when the call does.
/// A late send never delays the next one.
pub async fn run<'a, F, Fut, T>(requests: &'a [Planned], send: F) -> Vec<T>
where
    F: Fn(&'a Planned) -> Fut,
    Fut: Future<Output = T>,
{
    let t0 = tokio::time::Instant::now();
    let send = &send;
    futures::future::join_all(requests.iter().map(|r| async move {
        tokio::time::sleep_until(t0 + Duration::from_secs_f64(r.offset_ms / 1000.0)).await;
        send(r).await
    }))
    .await
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE: &str = include_str!("../tests/trace.jsonl");

    #[test]
    fn the_same_bytes_give_the_same_digest() {
        let a = Trace::parse(EXAMPLE.as_bytes()).unwrap();
        let b = Trace::parse(EXAMPLE.as_bytes()).unwrap();
        assert_eq!(a.digest, b.digest);
        assert_eq!(a.digest.len(), 64);
    }

    #[test]
    fn a_changed_offset_changes_the_digest() {
        let a = Trace::parse(EXAMPLE.as_bytes()).unwrap();
        let changed = EXAMPLE.replacen("\"offset_ms\": 250", "\"offset_ms\": 251", 1);
        assert_ne!(changed, EXAMPLE);
        let b = Trace::parse(changed.as_bytes()).unwrap();
        assert_ne!(a.digest, b.digest);
    }

    #[test]
    fn the_example_loads_with_its_sessions() {
        let t = Trace::parse(EXAMPLE.as_bytes()).unwrap();
        assert_eq!(t.requests.len(), 5);
        assert_eq!(t.sessions(), 2);
        let offsets: Vec<f64> = t.requests.iter().map(|r| r.offset_ms).collect();
        assert_eq!(offsets, vec![0.0, 100.0, 250.0, 400.0, 400.0]);
    }

    #[test]
    fn a_session_shares_a_prefix_that_grows() {
        let t = Trace::parse(EXAMPLE.as_bytes()).unwrap();
        let a: Vec<&Planned> = t
            .requests
            .iter()
            .filter(|r| r.session.as_deref() == Some("a"))
            .collect();
        assert_eq!(a.len(), 3);
        for pair in a.windows(2) {
            assert!(pair[1].prompt.starts_with(&pair[0].prompt));
            assert!(pair[1].prompt.len() > pair[0].prompt.len());
        }
        // A request with no session carries only its own text.
        let alone = t.requests.iter().find(|r| r.session.is_none()).unwrap();
        assert!(!alone.prompt.contains(JOINER));
        // The other session does not share the prefix of the first.
        let b = t
            .requests
            .iter()
            .find(|r| r.session.as_deref() == Some("b"))
            .unwrap();
        assert!(!b.prompt.starts_with(&a[0].prompt));
    }

    const AGENT: &str = include_str!("../tests/agent_trace.jsonl");

    #[test]
    fn an_agent_session_grows_its_prefix_request_after_request() {
        let t = Trace::parse(AGENT.as_bytes()).unwrap();
        assert_eq!(t.sessions(), 2);
        assert_eq!(t.requests[0].offset_ms, 0.0);
        let a: Vec<&Planned> = t
            .requests
            .iter()
            .filter(|r| r.session.as_deref() == Some("a"))
            .collect();
        assert_eq!(a.len(), 4);
        for pair in a.windows(2) {
            assert!(pair[1].prompt.starts_with(&pair[0].prompt));
            assert!(pair[1].prompt.len() > pair[0].prompt.len());
        }
        assert!(a[1].prompt.contains("assistant: I will search"));
        assert!(a[1].prompt.contains("tool: src/config.rs:12"));
    }

    #[test]
    fn a_bad_trace_is_refused_with_its_line() {
        let cases = [
            ("", "no request"),
            ("{\"offset_ms\": 0}\n", "line 1"),
            ("{\"offset_ms\": -1, \"prompt\": \"x\"}\n", "line 1"),
            (
                "{\"offset_ms\": 5, \"prompt\": \"x\"}\n{\"offset_ms\": 4, \"prompt\": \"y\"}\n",
                "line 2",
            ),
            (
                "{\"offset_ms\": 0, \"prompt\": \"x\", \"oops\": 1}\n",
                "line 1",
            ),
            (
                "{\"offset_ms\": 0, \"prompt\": \"x\", \"max_tokens\": 0}\n",
                "line 1",
            ),
            ("not json\n", "line 1"),
        ];
        for (text, needle) in cases {
            let e = Trace::parse(text.as_bytes()).unwrap_err();
            assert!(e.contains(needle), "{text:?} gave {e}");
        }
    }

    #[test]
    fn messages_are_joined_into_the_prompt() {
        let text = "{\"offset_ms\": 0, \"messages\": [{\"role\": \"user\", \"content\": \"hi\"}, {\"role\": \"assistant\", \"content\": \"yo\"}]}\n";
        let t = Trace::parse(text.as_bytes()).unwrap();
        assert_eq!(t.requests[0].prompt, "user: hi\nassistant: yo");
    }

    #[tokio::test]
    async fn requests_are_sent_at_their_offsets_whatever_the_answers() {
        let t = Trace::parse(EXAMPLE.as_bytes()).unwrap();
        let start = std::time::Instant::now();
        // The first request answers slowly: it must not delay the others.
        let sent = run(&t.requests, |r| async move {
            let at = start.elapsed().as_secs_f64() * 1000.0;
            if r.line == 1 {
                tokio::time::sleep(Duration::from_millis(300)).await;
            }
            (r.offset_ms, at)
        })
        .await;
        assert_eq!(sent.len(), 5);
        for (offset, at) in sent {
            assert!(at >= offset - 1.0, "sent at {at} before {offset}");
            assert!(at < offset + 150.0, "sent at {at} long after {offset}");
        }
    }

    #[test]
    fn the_record_names_the_trace_by_its_digest() {
        let t = Trace::parse(EXAMPLE.as_bytes()).unwrap();
        let s = t.settings();
        assert_eq!(s["trace_sha256"], t.digest.as_str());
        assert_eq!(s["requests"], 5);
        assert_eq!(s["sessions"], 2);
        assert_eq!(s["duration_ms"], 400.0);
    }
}
