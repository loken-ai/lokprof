//! How far an answer departs from a reference answer.

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

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(texts: &[&str]) -> Vec<String> {
        texts.iter().map(|t| (*t).to_string()).collect()
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
