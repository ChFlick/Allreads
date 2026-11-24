//! Word and non-word tokens, with their sentence and term key.

use super::sentences::sentence_ranges;
use super::LanguageRules;

/// One piece of text: a word, or the spaces and punctuation between words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    /// The term key, for words only.
    pub key: Option<String>,
    /// Index of the sentence this token belongs to, within its block.
    pub sentence: usize,
}

impl Token {
    /// True if this token is a word (it has a term key).
    #[must_use]
    pub fn is_word(&self) -> bool {
        self.key.is_some()
    }
}

/// Splits `text` into tokens. Joining every token's text gives `text` back.
#[must_use]
pub fn tokenize(text: &str, rules: &LanguageRules) -> Vec<Token> {
    let mut tokens = Vec::new();
    for (sentence, range) in sentence_ranges(text, rules).into_iter().enumerate() {
        let slice = &text[range];
        let mut last = 0;
        for word in rules.word_run.find_iter(slice) {
            if word.start() > last {
                tokens.push(non_word(&slice[last..word.start()], sentence));
            }
            for part in split_elision(word.as_str(), &rules.elisions) {
                tokens.push(Token {
                    text: part.to_string(),
                    key: Some(rules.key(part)),
                    sentence,
                });
            }
            last = word.end();
        }
        if last < slice.len() {
            tokens.push(non_word(&slice[last..], sentence));
        }
    }
    tokens
}

fn non_word(text: &str, sentence: usize) -> Token {
    Token {
        text: text.to_string(),
        key: None,
        sentence,
    }
}

/// Splits an elided prefix off a word: `l'homme` becomes `l'` and `homme`.
/// Only listed prefixes split, so `aujourd'hui` stays whole.
fn split_elision<'a>(word: &'a str, elisions: &[String]) -> Vec<&'a str> {
    let Some((index, apostrophe)) = word.char_indices().find(|&(_, c)| c == '\'' || c == '’')
    else {
        return vec![word];
    };
    let prefix = word[..index].to_lowercase();
    if elisions.contains(&prefix) {
        let split = index + apostrophe.len_utf8();
        vec![&word[..split], &word[split..]]
    } else {
        vec![word]
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_rules::{builtin, preset};
    use super::*;
    use std::fmt::Write;

    fn words(text: &str, code: &str) -> Vec<String> {
        tokenize(text, &builtin(code))
            .into_iter()
            .filter(Token::is_word)
            .map(|t| t.text)
            .collect()
    }

    /// One line per token: sentence, W for words, the text, and the key.
    fn render(text: &str, rules: &LanguageRules) -> String {
        let mut out = String::new();
        for token in tokenize(text, rules) {
            let kind = if token.is_word() { "W" } else { "-" };
            let key = token.key.unwrap_or_default();
            writeln!(out, "{} {kind} {:?} {key}", token.sentence, token.text).unwrap();
        }
        out
    }

    #[test]
    fn tokens_join_back_to_the_original_text() {
        let text = "L’homme s'arrêta : « Aujourd'hui, peut-être ? » Fin.";
        let joined: String = tokenize(text, &builtin("fr"))
            .into_iter()
            .map(|t| t.text)
            .collect();
        assert_eq!(joined, text);
    }

    #[test]
    fn french_elision_splits_listed_prefixes_only() {
        assert_eq!(
            words("l'homme jusqu'à aujourd'hui qu'il", "fr"),
            ["l'", "homme", "jusqu'", "à", "aujourd'hui", "qu'", "il"]
        );
    }

    #[test]
    fn curly_apostrophes_elide_and_share_the_straight_key() {
        let tokens = tokenize("L’Homme", &builtin("fr"));
        assert_eq!(tokens[0].text, "L’");
        assert_eq!(tokens[0].key.as_deref(), Some("l'"));
        assert_eq!(tokens[1].key.as_deref(), Some("homme"));
    }

    #[test]
    fn english_keeps_contractions_whole() {
        assert_eq!(words("don't stop", "en"), ["don't", "stop"]);
    }

    #[test]
    fn hyphenated_words_are_one_word() {
        assert_eq!(words("peut-être est-ce", "fr"), ["peut-être", "est-ce"]);
    }

    #[test]
    fn numbers_and_dashes_are_not_words() {
        assert_eq!(words("3,14 pages — oui", "fr"), ["pages", "oui"]);
    }

    #[test]
    fn non_word_runs_are_merged() {
        let tokens = tokenize("dit, « oui", &builtin("fr"));
        let texts: Vec<&str> = tokens.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(texts, ["dit", ", « ", "oui"]);
    }

    #[test]
    fn rtl_words_are_words() {
        let rules = LanguageRules::new(&preset("\u{0590}-\u{05FF}", ".!?", "")).unwrap();
        let texts: Vec<String> = tokenize("שלום עולם. מה שלומך?", &rules)
            .into_iter()
            .filter(Token::is_word)
            .map(|t| t.text)
            .collect();
        assert_eq!(texts, ["שלום", "עולם", "מה", "שלומך"]);
    }

    #[test]
    fn french_sample_snapshot() {
        let text = "L’homme s'arrêta : « Aujourd'hui, peut-être ? » Il lut 3,14 pages. \
                    M. Dupont rit... puis partit !";
        insta::assert_snapshot!(render(text, &builtin("fr")));
    }

    #[test]
    fn german_sample_snapshot() {
        let text = "Herr Müller kam am 3. Oktober. „Grüß Gott!“, sagte er.";
        insta::assert_snapshot!(render(text, &builtin("de")));
    }

    #[test]
    fn english_sample_snapshot() {
        let text = "Mr. Smith didn't wait. \"Well-known,\" he said: it's 3.14.";
        insta::assert_snapshot!(render(text, &builtin("en")));
    }
}
