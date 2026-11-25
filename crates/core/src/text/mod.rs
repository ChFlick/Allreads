//! The text pipeline: XHTML becomes blocks with inline styles, block text is
//! split into sentences, and sentences into word and non-word tokens.
//!
//! The sentence rules follow Lukaisu's `findLatinSentenceEnd`
//! (<https://github.com/HugoFara/lukaisu>, Unlicense), rewritten as a plain
//! scan over characters. See `NOTICE`.

pub mod blocks;
pub mod key;
pub mod sentences;
pub mod tokens;

use regex::Regex;

/// Version of the parser's output. Bump it whenever blocks, sentences or
/// tokens change, so cached chapters from older versions are parsed again.
pub const PARSER_VERSION: i64 = 1;

use crate::presets::LanguagePreset;

/// Collapses runs of ASCII whitespace to one space and trims the ends.
/// Non-breaking spaces (U+00A0) are left alone.
pub(crate) fn normalize_whitespace(text: &str) -> String {
    let mut normalized = String::with_capacity(text.len());
    for word in text
        .split(|c: char| c.is_ascii_whitespace())
        .filter(|word| !word.is_empty())
    {
        if !normalized.is_empty() {
            normalized.push(' ');
        }
        normalized.push_str(word);
    }
    normalized
}

/// A language's tokenizing rules, compiled once from its preset.
#[derive(Debug, Clone)]
pub struct LanguageRules {
    /// A word: word characters, optionally joined by one apostrophe or hyphen.
    word_run: Regex,
    /// Patterns like `Mr.` that never end a sentence.
    exceptions: Option<Regex>,
    end_chars: Vec<char>,
    /// Lower-case prefixes split off before an apostrophe, e.g. `l` in `l'homme`.
    elisions: Vec<String>,
    substitutions: Vec<(String, String)>,
}

impl LanguageRules {
    /// Compiles the rules in a preset.
    ///
    /// # Errors
    /// Returns a description of the first rule that is not a valid pattern.
    pub fn new(preset: &LanguagePreset) -> Result<Self, String> {
        let class = &preset.word_characters;
        if class.is_empty() {
            return Err("word_characters is empty".into());
        }
        // An unescaped bracket would close or open a class early. The pattern
        // would still compile, but with a different meaning.
        if has_unescaped_bracket(class) {
            return Err("word_characters must not contain an unescaped [ or ]".into());
        }
        let word_run = Regex::new(&format!("[{class}]+(?:['’-][{class}]+)*"))
            .map_err(|e| format!("word_characters is not a valid character class: {e}"))?;
        let exceptions = if preset.sentence_exceptions.trim().is_empty() {
            None
        } else {
            let pattern = format!("^(?:{})$", preset.sentence_exceptions);
            Some(
                Regex::new(&pattern)
                    .map_err(|e| format!("sentence_exceptions is not a valid pattern: {e}"))?,
            )
        };
        let elisions = preset
            .elision_prefixes
            .split('|')
            .map(|prefix| prefix.trim().to_lowercase())
            .filter(|prefix| !prefix.is_empty())
            .collect();
        Ok(Self {
            word_run,
            exceptions,
            end_chars: preset.sentence_end_chars.chars().collect(),
            elisions,
            substitutions: key::parse_substitutions(&preset.character_substitutions),
        })
    }

    /// The term key for a word or phrase in this language.
    #[must_use]
    pub fn key(&self, text: &str) -> String {
        key::term_key(text, &self.substitutions)
    }
}

/// True if `class` has a `[` or `]` that is not preceded by a backslash.
fn has_unescaped_bracket(class: &str) -> bool {
    let mut escaped = false;
    for c in class.chars() {
        match c {
            '\\' if !escaped => escaped = true,
            '[' | ']' if !escaped => return true,
            _ => escaped = false,
        }
    }
    false
}

#[cfg(test)]
pub(crate) mod test_rules {
    use super::LanguageRules;
    use crate::presets::{builtin_presets, parse_preset, LanguagePreset};

    /// The rules of a built-in preset, e.g. `"fr"`.
    pub(crate) fn builtin(code: &str) -> LanguageRules {
        let preset = builtin_presets()
            .unwrap()
            .into_iter()
            .find(|p| p.code == code)
            .expect("built-in preset exists");
        LanguageRules::new(&preset).unwrap()
    }

    /// A preset built from the three settings the parser tests vary.
    pub(crate) fn preset(
        word_characters: &str,
        end_chars: &str,
        exceptions: &str,
    ) -> LanguagePreset {
        let mut preset = parse_preset(
            r#"
            name = "Test"
            code = "xx"
            word_characters = "a-z"
            sentence_end_chars = "."
            sentence_exceptions = ""
            character_substitutions = ""
            rtl = false
            remove_spaces = false
            split_each_char = false
            tts_locale = "xx-XX"
            dictionary_url = ""
            translator_url = ""
            "#,
        )
        .unwrap();
        preset.word_characters = word_characters.into();
        preset.sentence_end_chars = end_chars.into();
        preset.sentence_exceptions = exceptions.into();
        preset
    }
}

#[cfg(test)]
mod tests {
    use super::test_rules::preset;
    use super::*;

    /// Lukaisu's golden fixtures, generated from its original PHP parser.
    ///
    /// We match their sentences, and their words with three documented
    /// differences: an abbreviation's dot is not part of the word (`Mr`, not
    /// `Mr.`), numbers are not words, and Hebrew words are words (the PHP
    /// parser got that wrong; Lukaisu's own tests correct it the same way).
    #[test]
    fn matches_lukaisu_parser_fixtures() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/lukaisu-parser-fixtures.json"
        );
        let fixtures: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let latin = "a-zA-ZÀ-ÖØ-öø-ȳ";
        let english = "Mr.|Mrs.|Dr.|[A-Z].";
        let cases = [
            ("english_basic", latin, english),
            ("english_abbrev", latin, english),
            ("english_paragraphs", latin, english),
            ("english_quotes", latin, english),
            ("french_basic", latin, "[A-Z].|Dr."),
            ("german_basic", "a-zA-ZäöüÄÖÜß", "[A-Z].|Dr."),
            ("hebrew_rtl", "\u{0590}-\u{05FF}", ""),
            ("numbers_mixed", latin, english),
        ];
        for (name, word_characters, exceptions) in cases {
            let case = &fixtures[name];
            let rules = LanguageRules::new(&preset(word_characters, ".!?:;", exceptions)).unwrap();
            let text = case["text"].as_str().unwrap();

            let ours: Vec<String> = sentences::sentence_ranges(text, &rules)
                .into_iter()
                .map(|range| text[range].trim().to_string())
                .collect();
            let expected: Vec<String> = case["result"]["sentences"]
                .as_array()
                .unwrap()
                .iter()
                .map(|s| s.as_str().unwrap().trim().to_string())
                .filter(|s| s != "¶")
                .collect();
            assert_eq!(ours, expected, "sentences of {name}");

            let our_words: Vec<String> = tokens::tokenize(text, &rules)
                .into_iter()
                .filter(tokens::Token::is_word)
                .map(|t| t.text)
                .collect();
            let expected_words: Vec<String> = case["result"]["tokens"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|t| t["w"].as_bool().unwrap() || name == "hebrew_rtl")
                .map(|t| {
                    t["text"]
                        .as_str()
                        .unwrap()
                        .trim()
                        .trim_end_matches('.')
                        .to_string()
                })
                .filter(|w| !w.is_empty() && !w.chars().any(|c| c.is_ascii_digit()))
                .filter(|w| w.chars().any(char::is_alphabetic))
                .collect();
            assert_eq!(our_words, expected_words, "words of {name}");
        }
    }

    #[test]
    fn every_built_in_preset_compiles() {
        for preset in crate::presets::builtin_presets().unwrap() {
            assert!(LanguageRules::new(&preset).is_ok(), "{} fails", preset.code);
        }
    }

    #[test]
    fn a_broken_character_class_is_reported() {
        let error = LanguageRules::new(&preset("a-z]", ".", "")).unwrap_err();
        assert!(error.contains("word_characters"), "{error}");
    }

    #[test]
    fn a_broken_exception_pattern_is_reported() {
        let error = LanguageRules::new(&preset("a-z", ".", "Mr.|(")).unwrap_err();
        assert!(error.contains("sentence_exceptions"), "{error}");
    }
}
