//! Replay of a recorded session: requests sent at the offsets a trace holds.
//!
//! A trace is a file of JSON lines, one request per line.
//! The arrivals are fixed by the file, whatever the engine has answered, like the offered load.
//! The record names the trace by the SHA-256 of its bytes, so 2 engines replayed on the same
//! trace are known to have been offered the same requests.
//!
//! 2 formats are read, told apart by the fields of the first line.
//!
//! The text format holds `offset_ms`, an optional `session`, a `prompt` or a list of `messages`
//! (`role` and `content`), and an optional `max_tokens`.
//! The requests of a session grow: the prompt sent is the parts of the session so far, joined
//! by an empty line.
//! A line of a session therefore holds the new parts only.
//!
//! The Mooncake format holds `timestamp` in milliseconds, `input_length` and `output_length`
//! in tokens, and `hash_ids`: 1 identifier per block of 512 prompt tokens.
//! 2 requests that share their first N identifiers share their first N blocks.
//! It carries no text: each block is a fixed sequence of words drawn from its identifier, 1 word
//! per token, so equal identifiers give an equal prefix.
//! The offset of a request is its `timestamp` minus the `timestamp` of the first line.
//!
//! `scripts/session_to_trace.py` writes a trace in the text format from the log of an agent
//! session.

use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::future::Future;
use std::time::Duration;

/// Joins the parts of a session into one prompt.
const JOINER: &str = "\n\n";

/// The tokens 1 hash of a Mooncake trace stands for, as the format defines it.
const MOONCAKE_BLOCK_TOKENS: usize = 512;

/// The words a synthesised block is drawn from.
/// Each is a common English word after a space, which tokenizers hold as 1 token.
const WORDS: [&str; 64] = [
    " the", " of", " and", " to", " in", " is", " for", " on", " that", " with", " as", " was",
    " at", " by", " it", " from", " be", " this", " are", " or", " an", " not", " but", " have",
    " his", " they", " one", " all", " were", " we", " can", " her", " has", " there", " their",
    " been", " if", " more", " when", " will", " would", " who", " so", " no", " she", " other",
    " its", " may", " these", " about", " than", " into", " time", " some", " could", " them",
    " only", " new", " first", " also", " any", " over", " after", " most",
];

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

/// One line of a Mooncake trace.
#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct MooncakeLine {
    /// Milliseconds of the arrival.
    timestamp: f64,
    /// Tokens of the prompt.
    input_length: usize,
    /// Tokens of the answer.
    output_length: usize,
    /// 1 identifier per block of the prompt, the last block possibly partial.
    hash_ids: Vec<u64>,
}

/// The format of a trace file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// `offset_ms` with the text of each request.
    Text,
    /// `timestamp` with the block hashes of each prompt.
    Mooncake,
}

impl Format {
    fn name(self) -> &'static str {
        match self {
            Format::Text => "text",
            Format::Mooncake => "mooncake",
        }
    }
}

/// What a request sends.
#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    /// The text as it goes.
    Text(String),
    /// Blocks drawn from their hashes, `tokens` words in all.
    Blocks { hash_ids: Vec<u64>, tokens: usize },
}

/// The next value of a SplitMix64 sequence.
fn splitmix64(state: &mut u64) -> u64 {
    *state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = *state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Appends the first `words` words of the block of `hash`.
/// The words of a block depend on its hash alone, so a shorter block is a prefix of a longer.
fn push_block(out: &mut String, hash: u64, words: usize) {
    let mut state = hash;
    for _ in 0..words {
        let i = (splitmix64(&mut state) >> 58) as usize;
        out.push_str(WORDS[i]);
    }
}

/// A request as it is sent.
#[derive(Debug, Clone, PartialEq)]
pub struct Planned {
    /// The line of the trace, from 1.
    pub line: usize,
    pub offset_ms: f64,
    pub session: Option<String>,
    /// The whole prompt: in a session, the earlier parts, then this part.
    pub body: Body,
    pub max_tokens: Option<usize>,
}

impl Planned {
    /// The text sent. Blocks are drawn at each call, so a long trace holds no prompt text.
    pub fn prompt(&self) -> Cow<'_, str> {
        match &self.body {
            Body::Text(t) => Cow::Borrowed(t),
            Body::Blocks { hash_ids, tokens } => {
                let mut out = String::with_capacity(tokens * 6);
                let mut left = *tokens;
                for &h in hash_ids {
                    let words = left.min(MOONCAKE_BLOCK_TOKENS);
                    push_block(&mut out, h, words);
                    left -= words;
                }
                Cow::Owned(out)
            }
        }
    }
}

/// A validated trace.
#[derive(Debug, Clone)]
pub struct Trace {
    /// SHA-256 of the bytes of the file, in hexadecimal.
    pub digest: String,
    pub format: Format,
    pub requests: Vec<Planned>,
}

impl Trace {
    pub fn load(path: &str) -> Result<Self, String> {
        let bytes = std::fs::read(path).map_err(|e| format!("cannot read the trace: {e}"))?;
        Self::parse(&bytes)
    }

    /// Reads a trace. A blank line is skipped.
    /// Any other line that is not valid is an error that names its number.
    /// A first line with a `timestamp` makes the file a Mooncake trace.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let text =
            std::str::from_utf8(bytes).map_err(|e| format!("the trace is not UTF-8: {e}"))?;
        let digest = format!("{:x}", Sha256::digest(bytes));
        let lines = text
            .lines()
            .enumerate()
            .map(|(i, raw)| (i + 1, raw))
            .filter(|(_, raw)| !raw.trim().is_empty());
        let first = lines.clone().next();
        let format = match first {
            Some((_, raw)) => match serde_json::from_str::<serde_json::Value>(raw) {
                Ok(v) if v.get("timestamp").is_some() => Format::Mooncake,
                _ => Format::Text,
            },
            None => Format::Text,
        };
        let requests = match format {
            Format::Text => parse_text(lines)?,
            Format::Mooncake => parse_mooncake(lines)?,
        };
        if requests.is_empty() {
            return Err("the trace holds no request".to_string());
        }
        Ok(Self {
            digest,
            format,
            requests,
        })
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
        let mut s = serde_json::json!({
            "trace_sha256": self.digest,
            "format": self.format.name(),
            "requests": self.requests.len(),
            "sessions": self.sessions(),
            "duration_ms": self.requests.last().map_or(0.0, |r| r.offset_ms),
        });
        if self.format == Format::Mooncake {
            let (blocks, reused) = self.blocks();
            s["block_tokens"] = MOONCAKE_BLOCK_TOKENS.into();
            s["blocks"] = blocks.into();
            s["reused_blocks"] = reused.into();
        }
        s
    }

    /// The blocks the requests send, and those of them an earlier request already sent.
    fn blocks(&self) -> (usize, usize) {
        let mut seen = HashSet::new();
        let mut blocks = 0;
        let mut reused = 0;
        for r in &self.requests {
            if let Body::Blocks { hash_ids, .. } = &r.body {
                for h in hash_ids {
                    blocks += 1;
                    if !seen.insert(*h) {
                        reused += 1;
                    }
                }
            }
        }
        (blocks, reused)
    }
}

/// Refuses an offset that is not a time or that goes back before the line before.
fn check_offset(
    n: usize,
    offset_ms: f64,
    last: Option<&Planned>,
    field: &str,
) -> Result<(), String> {
    if !offset_ms.is_finite() || offset_ms < 0.0 {
        return Err(format!("trace line {n}: {field} must be 0 or more"));
    }
    if let Some(last) = last {
        if offset_ms < last.offset_ms {
            return Err(format!(
                "trace line {n}: {field} goes back before line {}",
                last.line
            ));
        }
    }
    Ok(())
}

/// Reads the lines of a trace in the text format.
fn parse_text<'a>(lines: impl Iterator<Item = (usize, &'a str)>) -> Result<Vec<Planned>, String> {
    let mut requests: Vec<Planned> = Vec::new();
    // The text sent so far, by session.
    let mut history: BTreeMap<String, String> = BTreeMap::new();
    for (n, raw) in lines {
        let line: Line = serde_json::from_str(raw).map_err(|e| format!("trace line {n}: {e}"))?;
        check_offset(n, line.offset_ms, requests.last(), "offset_ms")?;
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
            body: Body::Text(prompt),
            max_tokens: line.max_tokens,
        });
    }
    Ok(requests)
}

/// Reads the lines of a Mooncake trace.
fn parse_mooncake<'a>(
    lines: impl Iterator<Item = (usize, &'a str)>,
) -> Result<Vec<Planned>, String> {
    let mut requests: Vec<Planned> = Vec::new();
    let mut start = None;
    for (n, raw) in lines {
        let line: MooncakeLine =
            serde_json::from_str(raw).map_err(|e| format!("trace line {n}: {e}"))?;
        if !line.timestamp.is_finite() || line.timestamp < 0.0 {
            return Err(format!("trace line {n}: timestamp must be 0 or more"));
        }
        let offset_ms = line.timestamp - *start.get_or_insert(line.timestamp);
        check_offset(n, offset_ms, requests.last(), "timestamp")?;
        if line.input_length == 0 || line.output_length == 0 {
            return Err(format!(
                "trace line {n}: input_length and output_length must be above 0"
            ));
        }
        if line.hash_ids.len() != line.input_length.div_ceil(MOONCAKE_BLOCK_TOKENS) {
            return Err(format!(
                "trace line {n}: {} hash_ids for {} tokens, 1 per block of {MOONCAKE_BLOCK_TOKENS} is expected",
                line.hash_ids.len(),
                line.input_length
            ));
        }
        requests.push(Planned {
            line: n,
            offset_ms,
            session: None,
            body: Body::Blocks {
                hash_ids: line.hash_ids,
                tokens: line.input_length,
            },
            max_tokens: Some(line.output_length),
        });
    }
    Ok(requests)
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
            assert!(pair[1].prompt().starts_with(&*pair[0].prompt()));
            assert!(pair[1].prompt().len() > pair[0].prompt().len());
        }
        // A request with no session carries only its own text.
        let alone = t.requests.iter().find(|r| r.session.is_none()).unwrap();
        assert!(!alone.prompt().contains(JOINER));
        // The other session does not share the prefix of the first.
        let b = t
            .requests
            .iter()
            .find(|r| r.session.as_deref() == Some("b"))
            .unwrap();
        assert!(!b.prompt().starts_with(&*a[0].prompt()));
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
            assert!(pair[1].prompt().starts_with(&*pair[0].prompt()));
            assert!(pair[1].prompt().len() > pair[0].prompt().len());
        }
        assert!(a[1].prompt().contains("assistant: I will search"));
        assert!(a[1].prompt().contains("tool: src/config.rs:12"));
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
        assert_eq!(t.requests[0].prompt(), "user: hi\nassistant: yo");
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

    /// The first 4 words of the block of hash 0.
    const PINNED_BLOCK_0: &str = " only all of after";

    const MOONCAKE: &str = include_str!("../tests/mooncake_toolagent.jsonl");

    /// The words of a prompt: each begins with the only space it holds.
    fn words(prompt: &str) -> Vec<&str> {
        prompt.split(' ').skip(1).collect()
    }

    #[test]
    fn a_mooncake_trace_is_told_apart_by_its_timestamp() {
        let t = Trace::parse(MOONCAKE.as_bytes()).unwrap();
        assert_eq!(t.format, Format::Mooncake);
        assert_eq!(t.requests.len(), 10);
        assert_eq!(t.sessions(), 0);
        // The offsets start at the first arrival and keep the recorded gaps.
        let offsets: Vec<f64> = t.requests.iter().map(|r| r.offset_ms).collect();
        assert_eq!(offsets[0], 0.0);
        assert_eq!(offsets[1], 66000.0);
        assert_eq!(offsets[9], 288000.0);
        assert_eq!(t.requests[0].max_tokens, Some(44));
        assert_eq!(
            Trace::parse(EXAMPLE.as_bytes()).unwrap().format,
            Format::Text
        );
    }

    #[test]
    fn a_mooncake_prompt_holds_1_word_per_token() {
        let t = Trace::parse(MOONCAKE.as_bytes()).unwrap();
        let lengths = [1069, 1141, 1205, 1101, 1311, 1292, 1475, 1649, 1362, 1850];
        for (r, n) in t.requests.iter().zip(lengths) {
            let p = r.prompt();
            assert_eq!(words(&p).len(), n, "line {}", r.line);
            assert!(words(&p)
                .iter()
                .all(|w| WORDS.contains(&format!(" {w}").as_str())));
        }
    }

    #[test]
    fn shared_hashes_give_a_shared_prefix() {
        let t = Trace::parse(MOONCAKE.as_bytes()).unwrap();
        let p: Vec<String> = t.requests.iter().map(|r| r.prompt().into_owned()).collect();
        let shared = |a: &str, b: &str| {
            words(a)
                .iter()
                .zip(words(b))
                .take_while(|(x, y)| *x == y)
                .count()
        };
        // Lines 1 and 2 share the hashes 0 and 111101: 2 blocks.
        assert!(shared(&p[0], &p[1]) >= 2 * MOONCAKE_BLOCK_TOKENS);
        assert!(shared(&p[0], &p[1]) < 2 * MOONCAKE_BLOCK_TOKENS + 32);
        // Lines 1 and 4 share the hash 0 only.
        assert!(shared(&p[0], &p[3]) >= MOONCAKE_BLOCK_TOKENS);
        assert!(shared(&p[0], &p[3]) < MOONCAKE_BLOCK_TOKENS + 32);
        // Lines 8 and 10 share 3 blocks, the third whole.
        assert!(shared(&p[7], &p[9]) >= 3 * MOONCAKE_BLOCK_TOKENS);
        // The same hash gives the same words in every trace.
        let again = Trace::parse(MOONCAKE.as_bytes()).unwrap();
        assert_eq!(again.requests[4].prompt(), p[4].as_str());
    }

    #[test]
    fn a_block_is_drawn_from_its_hash_alone() {
        let mut a = String::new();
        push_block(&mut a, 7, 512);
        let mut b = String::new();
        push_block(&mut b, 7, 100);
        assert!(a.starts_with(&b));
        let mut c = String::new();
        push_block(&mut c, 8, 512);
        assert_ne!(a, c);
        let mut d = String::new();
        push_block(&mut d, 0, 4);
        assert_eq!(d, PINNED_BLOCK_0);
    }

    #[test]
    fn the_record_holds_the_blocks_a_mooncake_trace_reuses() {
        let t = Trace::parse(MOONCAKE.as_bytes()).unwrap();
        let s = t.settings();
        assert_eq!(s["trace_sha256"], t.digest.as_str());
        assert_eq!(s["format"], "mooncake");
        assert_eq!(s["block_tokens"], 512);
        assert_eq!(s["blocks"], 32);
        assert_eq!(s["reused_blocks"], 18);
        assert_eq!(s["duration_ms"], 288000.0);
        let text = Trace::parse(EXAMPLE.as_bytes()).unwrap().settings();
        assert_eq!(text["format"], "text");
        assert!(text.get("blocks").is_none());
    }

    #[test]
    fn a_bad_mooncake_line_is_refused_with_its_line() {
        let ok = "{\"timestamp\": 5, \"input_length\": 600, \"output_length\": 4, \"hash_ids\": [1, 2]}\n";
        assert!(Trace::parse(ok.as_bytes()).is_ok());
        let cases = [
            "{\"timestamp\": 5, \"input_length\": 600, \"output_length\": 4, \"hash_ids\": [1]}\n",
            "{\"timestamp\": 5, \"input_length\": 600, \"output_length\": 0, \"hash_ids\": [1, 2]}\n",
            "{\"timestamp\": -1, \"input_length\": 600, \"output_length\": 4, \"hash_ids\": [1, 2]}\n",
            "{\"timestamp\": 5, \"input_length\": 600, \"output_length\": 4}\n",
            "{\"timestamp\": 5, \"input_length\": 600, \"output_length\": 4, \"hash_ids\": [1, 2], \"oops\": 1}\n",
        ];
        for text in cases {
            let e = Trace::parse(text.as_bytes()).unwrap_err();
            assert!(e.contains("line 1"), "{text:?} gave {e}");
        }
        let back = format!("{ok}{}", ok.replace("\"timestamp\": 5", "\"timestamp\": 4"));
        let e = Trace::parse(back.as_bytes()).unwrap_err();
        assert!(e.contains("line 2"), "{e}");
    }
}
