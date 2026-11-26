//! Counting a book's words for its difficulty: how often each word occurs,
//! which words are names, and which are forms of another word.

use std::collections::HashMap;

use crate::dictionary::WordKinds;
use crate::reader::ParsedChapter;
use crate::text::LanguageRules;

/// Bumped when the counting rules change, so books are counted again.
pub const TALLY_VERSION: i64 = 3;

/// One word of a book, ready to store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookWord {
    pub count: u32,
    /// The term key of its dictionary form, for a form like "lisait" (of "lire").
    pub lemma_key: Option<String>,
    /// A name, left out of the difficulty and not marked as new in the reader.
    pub is_name: bool,
}

/// How a word was written across the book.
#[derive(Debug, Default)]
struct Evidence {
    count: u32,
    /// Written in lower case somewhere ("pierre").
    lower: u32,
    /// Capitalised where a sentence does not start ("… dit Pierre").
    capital_inside: u32,
}

/// Languages that capitalise every noun, where capitals say nothing about names.
const CAPITALISED_NOUNS: [&str; 2] = ["de", "lb"];

/// Collects a book's words chapter by chapter.
#[derive(Debug)]
pub struct Tally {
    words: HashMap<String, Evidence>,
    /// Whether capitals may mark a name in this language (not in German).
    capitals_mark_names: bool,
}

impl Tally {
    /// A tally for a book in `language_code`.
    #[must_use]
    pub fn new(language_code: &str) -> Self {
        Self {
            words: HashMap::new(),
            capitals_mark_names: !CAPITALISED_NOUNS.contains(&language_code),
        }
    }

    /// Counts a chapter's words. Headings count, but say nothing about
    /// capitals: titles capitalise common words.
    pub fn add_chapter(&mut self, chapter: &ParsedChapter) {
        for block in &chapter.blocks {
            let heading = block.heading_level.is_some();
            let mut last_sentence = None;
            for token in &block.tokens {
                let Some(key) = &token.key else { continue };
                let starts_sentence = last_sentence != Some(token.sentence);
                last_sentence = Some(token.sentence);
                let evidence = self.words.entry(key.clone()).or_default();
                evidence.count += 1;
                if heading {
                    continue;
                }
                match token.text.chars().next() {
                    Some(c) if c.is_lowercase() => evidence.lower += 1,
                    Some(c) if c.is_uppercase() && !starts_sentence => evidence.capital_inside += 1,
                    _ => {}
                }
            }
        }
    }

    /// Decides names and dictionary forms. `lookup` asks the offline
    /// dictionary (`None` when none is installed).
    ///
    /// - A **name** is a word the dictionary knows only as a name, or one it
    ///   does not know that is always capitalised in the book, at least once
    ///   inside a sentence ("Gregor"). A word the dictionary knows as a common
    ///   word is never a name, even if the book always capitalises it.
    /// - A **form** is a word without meanings of its own in the dictionary
    ///   that is a form of another ("lisait" → "lire"). Words with their own
    ///   meanings ("lit", bed) keep their own count.
    pub fn finish(
        self,
        lookup: &dyn Fn(&str) -> Option<WordKinds>,
        rules: &LanguageRules,
    ) -> HashMap<String, BookWord> {
        let capitals_mark_names = self.capitals_mark_names;
        self.words
            .into_iter()
            .map(|(key, evidence)| {
                let kinds = lookup(&key).unwrap_or_default();
                // A common word: a form of something, or an entry that is not a name.
                let common = !kinds.lemmas.is_empty()
                    || kinds.parts_of_speech.iter().any(|pos| pos != "name");
                // "I" in English, or one-letter abbreviations, are not names.
                let always_capital = capitals_mark_names
                    && key.chars().count() > 1
                    && evidence.lower == 0
                    && evidence.capital_inside > 0;
                // Not common, and either the dictionary has it (only as a name) or the book always capitalises it.
                let is_name = !common && (!kinds.parts_of_speech.is_empty() || always_capital);
                // Only a form of exactly one word: "suis" (être, suivre) stays itself.
                let lemma_key = match kinds.lemmas.as_slice() {
                    [lemma] if kinds.parts_of_speech.is_empty() => Some(rules.term_key(lemma)),
                    _ => None,
                }
                .filter(|lemma| !lemma.is_empty() && *lemma != key);
                let word = BookWord {
                    count: evidence.count,
                    lemma_key,
                    is_name,
                };
                (key, word)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::{ParsedBlock, ParsedToken};
    use crate::text::test_rules::builtin;

    fn block(heading: bool, words: &[(&str, u32)]) -> ParsedBlock {
        ParsedBlock {
            heading_level: heading.then_some(1),
            tokens: words
                .iter()
                .map(|(text, sentence)| ParsedToken {
                    text: (*text).to_string(),
                    key: Some(text.to_lowercase()),
                    sentence: *sentence,
                    italic: false,
                    bold: false,
                })
                .collect(),
        }
    }

    fn kinds(parts: &[&str], lemmas: &[&str]) -> WordKinds {
        WordKinds {
            parts_of_speech: parts.iter().map(|p| (*p).to_string()).collect(),
            lemmas: lemmas.iter().map(|l| (*l).to_string()).collect(),
        }
    }

    /// A tiny dictionary: "paris" is a name, "coq" a noun, "lisait" a form of
    /// "lire", "suis" a form of both "être" and "suivre".
    fn lookup(key: &str) -> Option<WordKinds> {
        Some(match key {
            "paris" => kinds(&["name"], &[]),
            "coq" => kinds(&["noun"], &[]),
            "lisait" => kinds(&[], &["lire"]),
            "suis" => kinds(&[], &["suivre", "être"]),
            _ => WordKinds::default(),
        })
    }

    fn tally(blocks: Vec<ParsedBlock>) -> HashMap<String, BookWord> {
        let mut tally = Tally::new("fr");
        tally.add_chapter(&ParsedChapter { blocks });
        tally.finish(&lookup, &builtin("fr"))
    }
    #[test]
    fn capitals_inside_sentences_mark_unknown_names() {
        let words = tally(vec![
            block(true, &[("Le", 0), ("Coq", 0)]),
            block(
                false,
                &[
                    ("Gregor", 0),
                    ("dort", 0),
                    ("et", 0),
                    ("Gregor", 0),
                    ("lisait", 0),
                ],
            ),
            block(
                false,
                &[
                    ("Le", 0),
                    ("Coq", 0),
                    ("voit", 0),
                    ("Paris", 0),
                    ("Pierre", 0),
                    ("et", 0),
                    ("pierre", 0),
                ],
            ),
        ]);
        assert!(words["gregor"].is_name);
        // The dictionary knows it as a name only.
        assert!(words["paris"].is_name);
        // Always capitalised, but the dictionary knows it as a common word.
        assert!(!words["coq"].is_name);
        // Also written in lower case.
        assert!(!words["pierre"].is_name);
        // Capitalised only at sentence starts.
        assert!(!words["le"].is_name);
        assert_eq!(words["gregor"].count, 2);
        assert_eq!(words["le"].count, 2);
    }

    #[test]
    fn forms_point_to_their_only_dictionary_form() {
        let words = tally(vec![block(
            false,
            &[("il", 0), ("lisait", 0), ("je", 0), ("suis", 0)],
        )]);
        assert_eq!(words["lisait"].lemma_key.as_deref(), Some("lire"));
        assert_eq!(words["il"].lemma_key, None);
        // A form of two words is linked to neither.
        assert_eq!(words["suis"].lemma_key, None);
    }

    #[test]
    fn capitals_say_nothing_in_german_or_for_one_letter() {
        let mut german = Tally::new("de");
        german.add_chapter(&ParsedChapter {
            blocks: vec![block(
                false,
                &[
                    ("Der", 0),
                    ("Mann", 0),
                    ("sieht", 0),
                    ("das", 0),
                    ("Haus", 0),
                ],
            )],
        });
        let words = german.finish(&|_| None, &builtin("de"));
        assert!(!words["haus"].is_name);
        let words = tally(vec![block(false, &[("Alors", 0), ("A", 0), ("dit", 0)])]);
        assert!(!words["a"].is_name);
    }

    #[test]
    fn without_a_dictionary_only_capitals_decide() {
        let mut tally = Tally::new("fr");
        tally.add_chapter(&ParsedChapter {
            blocks: vec![block(false, &[("Alors", 0), ("Nemo", 0), ("parla", 0)])],
        });
        let words = tally.finish(&|_| None, &builtin("fr"));
        assert!(words["nemo"].is_name);
        assert!(!words["alors"].is_name);
        assert_eq!(words["parla"].lemma_key, None);
    }
}
