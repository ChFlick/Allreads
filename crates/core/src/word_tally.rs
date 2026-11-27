//! Counting a book's words for its difficulty: how often each word occurs,
//! which words are names, and which are forms of another word.

use std::collections::HashMap;

use crate::book::BookError;
use crate::dictionary::{DictionaryError, WordKinds};
use crate::library::OpenedBook;
use crate::reader::{parse_blocks, ParsedChapter};
use crate::text::{capitalises_nouns, LanguageRules, PARSER_VERSION};

/// Bumped when the counting rules change, so books are counted again.
pub const TALLY_VERSION: i64 = 3;

/// What a count depends on: the parser, the counting rules, and the offline
/// dictionary (its pair and build date, or `None`). A book whose stored
/// stamp differs is counted again, e.g. after a dictionary is installed.
#[must_use]
pub fn count_stamp(dictionary: Option<&str>) -> String {
    format!(
        "{PARSER_VERSION}.{TALLY_VERSION}/{}",
        dictionary.unwrap_or("")
    )
}

/// How [`count_stamp`] names an offline dictionary: its pair and build date,
/// e.g. `fr-en 2026-11-25`. Shared by the app and the catalog builder, so a
/// catalog says whether it was counted like the reader's library books.
#[must_use]
pub fn dictionary_identity(language_code: &str, built: Option<&str>) -> String {
    format!(
        "{}-en {}",
        language_code.to_lowercase(),
        built.unwrap_or_default()
    )
}

/// Why a book could not be counted.
#[derive(Debug, thiserror::Error)]
pub enum CountError {
    #[error(transparent)]
    Book(#[from] BookError),
    #[error(transparent)]
    Dictionary(#[from] DictionaryError),
}

/// Asks the offline dictionary about a word; `None` when none is installed.
pub type Lookup<'a> = Option<&'a dyn Fn(&str) -> Result<WordKinds, DictionaryError>>;

/// Counts every word of a book with its first `chapters` chapters: how often
/// each occurs, which are names, which are forms of another word.
///
/// # Errors
/// Returns `Book` if a chapter cannot be read, and `Dictionary` on the first
/// failed lookup (the count is then incomplete and must not be stored).
pub fn count_book(
    book: &OpenedBook,
    chapters: usize,
    rules: &LanguageRules,
    language_code: &str,
    lookup: Lookup,
) -> Result<HashMap<String, BookWord>, CountError> {
    let mut tally = Tally::new(language_code);
    for index in 0..chapters {
        tally.add_chapter(&parse_blocks(&book.chapter_blocks(index)?, rules));
    }
    Ok(tally.finish(lookup, rules)?)
}

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
            capitals_mark_names: !capitalises_nouns(language_code),
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
    ///
    /// # Errors
    /// Returns the first dictionary error; the result would be incomplete.
    pub fn finish(
        self,
        lookup: Lookup,
        rules: &LanguageRules,
    ) -> Result<HashMap<String, BookWord>, DictionaryError> {
        let capitals_mark_names = self.capitals_mark_names;
        self.words
            .into_iter()
            .map(|(key, evidence)| {
                let kinds = match lookup {
                    Some(lookup) => lookup(&key)?,
                    None => WordKinds::default(),
                };
                // "I" in English, or one-letter abbreviations, are not names.
                let always_capital = capitals_mark_names
                    && key.chars().count() > 1
                    && evidence.lower == 0
                    && evidence.capital_inside > 0;
                // The same rule as for saving (`common_form`): a common word is never a name.
                let is_name = kinds.is_name_only() || (!kinds.is_common() && always_capital);
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
                Ok((key, word))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::{ParsedBlock, ParsedToken};
    use crate::text::test_rules::builtin;
    use std::path::Path;

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
    fn lookup(key: &str) -> Result<WordKinds, DictionaryError> {
        Ok(match key {
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
        tally.finish(Some(&lookup), &builtin("fr")).unwrap()
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
        let words = german.finish(None, &builtin("de")).unwrap();
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
        let words = tally.finish(None, &builtin("fr")).unwrap();
        assert!(words["nemo"].is_name);
        assert!(!words["alors"].is_name);
        assert_eq!(words["parla"].lemma_key, None);
    }

    #[test]
    fn a_whole_book_is_counted_and_a_lookup_error_stops_it() {
        let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny.epub");
        let book = OpenedBook::open(&fixture).unwrap();
        let chapters = book.chapter_titles().unwrap().len();
        let rules = builtin("fr");
        let words = count_book(&book, chapters, &rules, "fr", None).unwrap();
        assert!(!words.is_empty());
        let failing = |_: &str| -> Result<WordKinds, DictionaryError> {
            Err(DictionaryError::Invalid("broken".into()))
        };
        assert!(matches!(
            count_book(&book, chapters, &rules, "fr", Some(&failing)),
            Err(CountError::Dictionary(_))
        ));
        assert_eq!(
            count_stamp(Some("fr-en 2026-11-26")),
            format!("{PARSER_VERSION}.{TALLY_VERSION}/fr-en 2026-11-26")
        );
    }
}
