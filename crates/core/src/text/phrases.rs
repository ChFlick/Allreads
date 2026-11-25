//! Phrases: saved terms of several words, found again wherever they appear.
//!
//! A phrase's key is its words' keys joined by spaces, except straight after
//! an elided word ("d'" + "une" + "pierre" is "d'une pierre"), so it matches
//! what the reader's tokens give whatever spaces or punctuation sit between
//! the words.

/// The longest phrase looked for, in words.
pub const MAX_PHRASE_WORDS: usize = 6;

/// Joins word keys into a phrase key: a space between words, none after an
/// elided word that ends in an apostrophe.
#[must_use]
pub fn join_keys(keys: &[&str]) -> String {
    let mut joined = String::new();
    for key in keys {
        if !joined.is_empty() && !joined.ends_with('\'') {
            joined.push(' ');
        }
        joined.push_str(key);
    }
    joined
}

/// A token for phrase matching: its key if it is a word, and its sentence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WordSlot<'a> {
    pub key: Option<&'a str>,
    pub sentence: u32,
}

/// One phrase found in a run of tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhraseSpan {
    /// Index of its first and last word token.
    pub first: usize,
    pub last: usize,
    pub key: String,
}

/// Finds the phrases in `tokens` for which `is_phrase` says yes. Longer
/// phrases win over shorter ones, phrases do not overlap, and a phrase never
/// crosses a sentence boundary.
pub fn find_phrases(tokens: &[WordSlot], is_phrase: &dyn Fn(&str) -> bool) -> Vec<PhraseSpan> {
    // Positions and keys of the word tokens only.
    let words: Vec<(usize, &str, u32)> = tokens
        .iter()
        .enumerate()
        .filter_map(|(i, t)| t.key.map(|key| (i, key, t.sentence)))
        .collect();
    let mut spans = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let mut matched = 0;
        let longest = MAX_PHRASE_WORDS.min(words.len() - i);
        for n in (2..=longest).rev() {
            let run = &words[i..i + n];
            if run.iter().any(|w| w.2 != run[0].2) {
                continue;
            }
            let keys: Vec<&str> = run.iter().map(|w| w.1).collect();
            let key = join_keys(&keys);
            if is_phrase(&key) {
                spans.push(PhraseSpan {
                    first: run[0].0,
                    last: run[n - 1].0,
                    key,
                });
                matched = n;
                break;
            }
        }
        i += matched.max(1);
    }
    spans
}

/// Every run of 2 to [`MAX_PHRASE_WORDS`] words that contains word `index`,
/// as (first word, last word, phrase key), longest first. For looking up
/// phrase suggestions around a tapped word.
#[must_use]
pub fn runs_around(keys: &[&str], index: usize) -> Vec<(usize, usize, String)> {
    let mut runs = Vec::new();
    if index >= keys.len() {
        return runs;
    }
    for n in (2..=MAX_PHRASE_WORDS.min(keys.len())).rev() {
        let earliest = index.saturating_sub(n - 1);
        for first in earliest..=index {
            let last = first + n - 1;
            if last < keys.len() {
                runs.push((first, last, join_keys(&keys[first..=last])));
            }
        }
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slots<'a>(keys: &[Option<&'a str>]) -> Vec<WordSlot<'a>> {
        keys.iter()
            .map(|&key| WordSlot { key, sentence: 0 })
            .collect()
    }

    #[test]
    fn keys_join_without_a_space_after_an_elision() {
        assert_eq!(join_keys(&["d'", "une", "pierre"]), "d'une pierre");
        assert_eq!(join_keys(&["pierre", "précieuse"]), "pierre précieuse");
        assert_eq!(join_keys(&["chat"]), "chat");
    }

    #[test]
    fn a_phrase_is_found_across_spaces_and_punctuation() {
        // "une pierre, précieuse" with the comma and spaces as non-word tokens.
        let tokens = slots(&[Some("une"), None, Some("pierre"), None, Some("précieuse")]);
        let found = find_phrases(&tokens, &|k| k == "pierre précieuse");
        assert_eq!(
            found,
            [PhraseSpan {
                first: 2,
                last: 4,
                key: "pierre précieuse".into()
            }]
        );
    }

    #[test]
    fn the_longest_phrase_wins_and_phrases_do_not_overlap() {
        let tokens = slots(&[
            Some("tout"),
            None,
            Some("à"),
            None,
            Some("fait"),
            None,
            Some("fait"),
        ]);
        let phrases = ["tout à", "tout à fait", "fait fait"];
        let found = find_phrases(&tokens, &|k| phrases.contains(&k));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].key, "tout à fait");
    }

    #[test]
    fn a_phrase_does_not_cross_sentences() {
        let tokens = [
            WordSlot {
                key: Some("pierre"),
                sentence: 0,
            },
            WordSlot {
                key: None,
                sentence: 0,
            },
            WordSlot {
                key: Some("précieuse"),
                sentence: 1,
            },
        ];
        assert!(find_phrases(&tokens, &|_| true).is_empty());
    }

    #[test]
    fn runs_around_a_word_cover_every_window() {
        let runs = runs_around(&["une", "pierre", "précieuse"], 1);
        let keys: Vec<&str> = runs.iter().map(|r| r.2.as_str()).collect();
        assert_eq!(
            keys,
            ["une pierre précieuse", "une pierre", "pierre précieuse"]
        );
        assert!(runs_around(&["a"], 3).is_empty());
    }
}
