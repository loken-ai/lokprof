//! How far an answer departs from a reference answer.

use serde::{Deserialize, Deserializer, Serialize};

/// One generated token, its log-probability and the most likely tokens at its position.
///
/// The shape is an entry of OpenAI's `logprobs.content[]`.
/// Ollama and loken send the same entries in `logprobs[]` of `/api/generate`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TokenLogprob {
    pub token: String,
    pub logprob: f64,
    /// The most likely tokens at this position, as many as the request asked for.
    #[serde(
        default,
        deserialize_with = "null_as_empty",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub top_logprobs: Vec<TopLogprob>,
}

/// One of the most likely tokens at a position.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TopLogprob {
    pub token: String,
    pub logprob: f64,
}

/// Reads a `null` list as an empty one.
fn null_as_empty<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<TopLogprob>, D::Error> {
    Ok(Option::<Vec<TopLogprob>>::deserialize(d)?.unwrap_or_default())
}

/// The departure of one answer from the reference answer of the same iteration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Departure {
    /// The answer equals the reference, character for character.
    pub identical: bool,
    /// The length, in characters, of the longest prefix both texts share.
    pub common_prefix_chars: usize,
    /// The length of the reference, in characters.
    pub reference_chars: usize,
    /// The length of the answer, in characters.
    pub answer_chars: usize,
}

/// Compares an answer with the reference. Lengths and the prefix count characters, not bytes.
pub fn departure(reference: &str, answer: &str) -> Departure {
    let common_prefix_chars = reference
        .chars()
        .zip(answer.chars())
        .take_while(|(r, a)| r == a)
        .count();
    Departure {
        identical: reference == answer,
        common_prefix_chars,
        reference_chars: reference.chars().count(),
        answer_chars: answer.chars().count(),
    }
}

/// The `quality` value of a cell: its answers against the reference answers, pair by pair
/// in iteration order.
///
/// Only the first `min(reference.len(), answers.len())` iterations are compared.
/// It is `null` when no pair exists.
///
/// `reference_target` is the label of the target whose answers are the reference.
/// The value names it, so a record says which run the answers are compared with.
pub fn cell_quality(
    reference: &[String],
    answers: &[String],
    is_reference: bool,
    reference_target: &str,
) -> serde_json::Value {
    let departures: Vec<Departure> = reference
        .iter()
        .zip(answers)
        .map(|(r, a)| departure(r, a))
        .collect();
    if departures.is_empty() {
        return serde_json::Value::Null;
    }
    let identical = departures.iter().filter(|d| d.identical).count();
    let prefix_sum: usize = departures.iter().map(|d| d.common_prefix_chars).sum();
    serde_json::json!({
        "reference": is_reference,
        "reference_target": reference_target,
        "identical_iterations": identical,
        "iterations": departures.len(),
        "mean_common_prefix_chars": prefix_sum as f64 / departures.len() as f64,
    })
}

/// The most likely token at a position.
///
/// It is the first of the alternatives with the highest log-probability.
/// Without alternatives it is the generated token: the bench decodes greedily.
fn top_token(entry: &TokenLogprob) -> &str {
    entry
        .top_logprobs
        .iter()
        .reduce(|best, t| if t.logprob > best.logprob { t } else { best })
        .map_or(entry.token.as_str(), |t| t.token.as_str())
}

/// What 1 answer and its reference give, token by token.
#[derive(Debug, Default, PartialEq)]
struct TokenDeparture {
    /// `p - p_ref` of each token both answers share from the start.
    delta_p: Vec<f64>,
    /// `ln p_ref - ln p` of the same tokens.
    ln_ratio: Vec<f64>,
    /// The positions whose preceding tokens both answers share.
    positions: usize,
    /// Those of the positions whose most likely token is the same on both sides.
    same_top: usize,
}

/// Compares the log-probabilities of an answer with those of its reference.
///
/// The tokens both answers share from the start give `delta_p` and `ln_ratio`.
/// The positions up to the first token that differs, that one included, give `same_top`.
/// Past that position the 2 answers condition on different texts and nothing is compared.
fn token_departure(reference: &[TokenLogprob], answer: &[TokenLogprob]) -> TokenDeparture {
    let shared = reference
        .iter()
        .zip(answer)
        .take_while(|(r, a)| r.token == a.token)
        .count();
    let mut d = TokenDeparture::default();
    for (r, a) in reference.iter().zip(answer).take(shared) {
        d.delta_p.push(a.logprob.exp() - r.logprob.exp());
        d.ln_ratio.push(r.logprob - a.logprob);
    }
    for (r, a) in reference.iter().zip(answer).take(shared + 1) {
        d.positions += 1;
        if top_token(r) == top_token(a) {
            d.same_top += 1;
        }
    }
    d
}

/// The mean, the root mean square, the median and the extremes of `values`.
/// `null` when there is none.
fn spread(values: &[f64]) -> serde_json::Value {
    if values.is_empty() {
        return serde_json::Value::Null;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    let median = if n % 2 == 1 {
        sorted[n / 2]
    } else {
        (sorted[n / 2 - 1] + sorted[n / 2]) / 2.0
    };
    serde_json::json!({
        "mean": sorted.iter().sum::<f64>() / n as f64,
        "rms": (sorted.iter().map(|v| v * v).sum::<f64>() / n as f64).sqrt(),
        "median": median,
        "minimum": sorted[0],
        "maximum": sorted[n - 1],
    })
}

/// The departure of the log-probabilities of a cell from those of the reference cell, pooled
/// over the iterations that pair an answer with a reference, both with log-probabilities.
///
/// The statistics are those of a KL divergence run of a perplexity tool, restricted to what
/// the top log-probabilities of 2 free answers give:
///
/// - `shared_tokens`: the tokens both answers share from the start.
/// - `delta_p`: `p - p_ref` of each shared token.
/// - `mean_ln_ppl_ratio`: the mean of `ln p_ref - ln p` over the shared tokens.
/// - `same_top_p`: the share of the positions with a shared context whose most likely token
///   is the same.
///
/// No divergence over the vocabulary is given: the engines send the top tokens only.
///
/// `Err` holds the reason when no iteration pairs log-probabilities.
pub fn logprob_quality(
    reference: &[Option<Vec<TokenLogprob>>],
    answers: &[Option<Vec<TokenLogprob>>],
) -> Result<serde_json::Value, &'static str> {
    if !answers.iter().any(Option::is_some) {
        return Err("the engine sent no log-probabilities");
    }
    if !reference.iter().any(Option::is_some) {
        return Err("the reference holds no log-probabilities");
    }
    let mut pooled = TokenDeparture::default();
    let mut pairs = 0usize;
    for (r, a) in reference.iter().zip(answers) {
        let (Some(r), Some(a)) = (r, a) else {
            continue;
        };
        let d = token_departure(r, a);
        pooled.delta_p.extend(d.delta_p);
        pooled.ln_ratio.extend(d.ln_ratio);
        pooled.positions += d.positions;
        pooled.same_top += d.same_top;
        pairs += 1;
    }
    if pairs == 0 {
        return Err("no iteration pairs an answer with a reference, both with log-probabilities");
    }
    let same_top_p =
        (pooled.positions > 0).then(|| pooled.same_top as f64 / pooled.positions as f64);
    let mean_ln_ppl_ratio = (!pooled.ln_ratio.is_empty())
        .then(|| pooled.ln_ratio.iter().sum::<f64>() / pooled.ln_ratio.len() as f64);
    Ok(serde_json::json!({
        "iterations": pairs,
        "shared_tokens": pooled.delta_p.len(),
        "positions": pooled.positions,
        "same_top": pooled.same_top,
        "same_top_p": same_top_p,
        "mean_ln_ppl_ratio": mean_ln_ppl_ratio,
        "delta_p": spread(&pooled.delta_p),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(token: &str, logprob: f64, top: &[(&str, f64)]) -> TokenLogprob {
        TokenLogprob {
            token: token.to_string(),
            logprob,
            top_logprobs: top
                .iter()
                .map(|(t, l)| TopLogprob {
                    token: (*t).to_string(),
                    logprob: *l,
                })
                .collect(),
        }
    }

    #[test]
    fn the_shared_tokens_give_delta_p_and_the_ln_ratio() {
        let half = 0.5f64.ln();
        let quarter = 0.25f64.ln();
        let reference = [
            entry("a", half, &[]),
            entry("b", half, &[]),
            entry("c", half, &[]),
        ];
        let answer = [
            entry("a", quarter, &[]),
            entry("b", half, &[]),
            entry("x", half, &[]),
        ];
        let d = token_departure(&reference, &answer);
        assert_eq!(d.delta_p.len(), 2);
        assert!((d.delta_p[0] + 0.25).abs() < 1e-12);
        assert!(d.delta_p[1].abs() < 1e-12);
        assert!((d.ln_ratio[0] - 2f64.ln()).abs() < 1e-12);
        assert_eq!(d.positions, 3, "the first token that differs is compared");
        assert_eq!(d.same_top, 2);
    }

    #[test]
    fn the_top_token_comes_from_the_alternatives() {
        let reference = [entry("a", -1.0, &[("b", -0.5), ("a", -1.0)])];
        let answer = [entry("a", -0.2, &[("a", -0.2), ("b", -1.9)])];
        let d = token_departure(&reference, &answer);
        assert_eq!(d.positions, 1);
        assert_eq!(d.same_top, 0);
        assert_eq!(d.delta_p.len(), 1);
    }

    #[test]
    fn answers_that_differ_at_once_share_one_position() {
        let d = token_departure(&[entry("a", -0.1, &[])], &[entry("b", -0.1, &[])]);
        assert!(d.delta_p.is_empty());
        assert_eq!((d.positions, d.same_top), (1, 0));
    }

    #[test]
    fn the_cell_pools_its_iterations() {
        let half = 0.5f64.ln();
        let r = Some(vec![entry("a", half, &[]), entry("b", half, &[])]);
        let a = Some(vec![entry("a", half, &[]), entry("c", half, &[])]);
        let q = logprob_quality(&[r.clone(), r.clone(), r], &[a.clone(), None, a])
            .expect("2 iterations pair log-probabilities");
        assert_eq!(q["iterations"], 2);
        assert_eq!(q["shared_tokens"], 2);
        assert_eq!(q["positions"], 4);
        assert_eq!(q["same_top"], 2);
        assert_eq!(q["same_top_p"], 0.5);
        assert_eq!(q["mean_ln_ppl_ratio"], 0.0);
        assert_eq!(q["delta_p"]["maximum"], 0.0);
    }

    #[test]
    fn the_spread_reads_like_the_perplexity_tool() {
        let s = spread(&[0.3, -0.1, 0.1, -0.3]);
        assert!((s["mean"].as_f64().unwrap()).abs() < 1e-12);
        assert!((s["rms"].as_f64().unwrap() - 0.05f64.sqrt()).abs() < 1e-12);
        assert!((s["median"].as_f64().unwrap()).abs() < 1e-12);
        assert_eq!(s["minimum"], -0.3);
        assert_eq!(s["maximum"], 0.3);
        assert!(spread(&[]).is_null());
    }

    #[test]
    fn a_side_without_log_probabilities_says_why() {
        let some = Some(vec![entry("a", -0.1, &[])]);
        assert_eq!(
            logprob_quality(std::slice::from_ref(&some), &[None]),
            Err("the engine sent no log-probabilities")
        );
        assert_eq!(
            logprob_quality(&[None], std::slice::from_ref(&some)),
            Err("the reference holds no log-probabilities")
        );
        assert!(logprob_quality(&[some.clone(), None], &[None, some]).is_err());
    }

    fn owned(texts: &[&str]) -> Vec<String> {
        texts.iter().map(|t| (*t).to_string()).collect()
    }

    #[test]
    fn an_entry_reads_with_or_without_its_alternatives() {
        let full: TokenLogprob = serde_json::from_str(
            r#"{"token":" a","logprob":-0.5,"bytes":[32,97],
                "top_logprobs":[{"token":" a","logprob":-0.5,"bytes":[32,97]}]}"#,
        )
        .expect("an entry with alternatives");
        assert_eq!(full.top_logprobs.len(), 1);
        let bare: TokenLogprob =
            serde_json::from_str(r#"{"token":" a","logprob":-0.5}"#).expect("a bare entry");
        assert!(bare.top_logprobs.is_empty());
        let null: TokenLogprob =
            serde_json::from_str(r#"{"token":" a","logprob":-0.5,"top_logprobs":null}"#)
                .expect("an entry with null alternatives");
        assert!(null.top_logprobs.is_empty());
        assert_eq!(
            serde_json::to_value(&bare).expect("serialise"),
            serde_json::json!({"token": " a", "logprob": -0.5})
        );
    }

    #[test]
    fn identical_answers_depart_by_nothing() {
        let d = departure("hello", "hello");
        assert!(d.identical);
        assert_eq!(d.common_prefix_chars, 5);
        assert_eq!(d.reference_chars, 5);
        assert_eq!(d.answer_chars, 5);
    }

    #[test]
    fn a_shorter_answer_that_is_a_prefix_is_not_identical() {
        let d = departure("hello world", "hello");
        assert!(!d.identical);
        assert_eq!(d.common_prefix_chars, 5);
        assert_eq!(d.reference_chars, 11);
        assert_eq!(d.answer_chars, 5);
    }

    #[test]
    fn a_longer_answer_keeps_the_whole_reference_as_prefix() {
        let d = departure("hello", "hello world");
        assert!(!d.identical);
        assert_eq!(d.common_prefix_chars, 5);
        assert_eq!(d.reference_chars, 5);
        assert_eq!(d.answer_chars, 11);
    }

    #[test]
    fn divergence_in_the_middle_counts_characters_not_bytes() {
        let d = departure("caf\u{e9} noir", "caf\u{e9} rose");
        assert!(!d.identical);
        assert_eq!(d.common_prefix_chars, 5);
        assert_eq!(d.reference_chars, 9);
        assert_eq!(d.answer_chars, 9);
        let d = departure("\u{65e5}\u{672c}\u{8a9e}", "\u{65e5}\u{672c}\u{4eba}");
        assert_eq!(d.common_prefix_chars, 2);
        assert_eq!(d.reference_chars, 3);
    }

    #[test]
    fn an_empty_answer_shares_nothing() {
        let d = departure("hello", "");
        assert!(!d.identical);
        assert_eq!(d.common_prefix_chars, 0);
        assert_eq!(d.reference_chars, 5);
        assert_eq!(d.answer_chars, 0);
    }

    #[test]
    fn two_empty_texts_are_identical() {
        let d = departure("", "");
        assert!(d.identical);
        assert_eq!(d.common_prefix_chars, 0);
        assert_eq!(d.reference_chars, 0);
        assert_eq!(d.answer_chars, 0);
    }

    #[test]
    fn the_cell_aggregates_its_iterations() {
        let reference = owned(&["abcd", "abcd", "abcd"]);
        let answers = owned(&["abcd", "abxx", "ab"]);
        let q = cell_quality(&reference, &answers, false, "a");
        assert_eq!(q["reference"], false);
        assert_eq!(q["reference_target"], "a");
        assert_eq!(q["identical_iterations"], 1);
        assert_eq!(q["iterations"], 3);
        assert_eq!(q["mean_common_prefix_chars"], 8.0 / 3.0);
    }

    #[test]
    fn the_reference_cell_is_identical_to_itself() {
        let texts = owned(&["one", "two"]);
        let q = cell_quality(&texts, &texts, true, "a");
        assert_eq!(q["reference"], true);
        assert_eq!(q["identical_iterations"], 2);
        assert_eq!(q["iterations"], 2);
        assert_eq!(q["mean_common_prefix_chars"], 3.0);
    }

    #[test]
    fn unequal_counts_compare_the_shared_iterations_only() {
        let q = cell_quality(&owned(&["a", "b", "c"]), &owned(&["a"]), false, "a");
        assert_eq!(q["iterations"], 1);
        assert_eq!(q["identical_iterations"], 1);
    }

    #[test]
    fn a_cell_without_a_pair_has_no_quality() {
        assert!(cell_quality(&owned(&["a"]), &[], false, "a").is_null());
        assert!(cell_quality(&[], &owned(&["a"]), false, "a").is_null());
    }
}
