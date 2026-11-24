//! Term keys: the one normalised form used wherever words are matched.

use unicode_normalization::UnicodeNormalization;

/// The key for a word or phrase: Unicode NFC, then the language's character
/// substitutions, then lower case, with whitespace collapsed to single spaces.
///
/// NFC matters because an EPUB may store "é" as "e" plus a combining accent.
#[must_use]
pub fn term_key(text: &str, substitutions: &[(String, String)]) -> String {
    let mut key: String = text.nfc().collect();
    for (from, to) in substitutions {
        key = key.replace(from.as_str(), to);
    }
    let lower: String = key.to_lowercase().nfc().collect();
    lower.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Parses a preset's `from=to|from=to` substitutions, skipping malformed pairs.
#[must_use]
pub fn parse_substitutions(rules: &str) -> Vec<(String, String)> {
    rules
        .split('|')
        .filter_map(|pair| pair.split_once('='))
        .filter(|(from, _)| !from.is_empty())
        .map(|(from, to)| (from.to_string(), to.to_string()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn french() -> Vec<(String, String)> {
        parse_substitutions("´='|`='|’='|‘='|...=…|..=‥")
    }

    #[test]
    fn case_is_folded() {
        assert_eq!(term_key("Le", &[]), "le");
        assert_eq!(term_key("ÉTÉ", &[]), "été");
    }

    #[test]
    fn curly_and_straight_apostrophes_match() {
        assert_eq!(
            term_key("l’homme", &french()),
            term_key("l'homme", &french())
        );
    }

    #[test]
    fn decomposed_accents_match_composed_ones() {
        let decomposed = "e\u{301}te\u{301}"; // "été" as letters plus combining accents
        assert_eq!(term_key(decomposed, &[]), "été");
    }

    #[test]
    fn whitespace_is_trimmed_and_collapsed_for_phrases() {
        assert_eq!(term_key("  tout\u{a0} à   coup ", &[]), "tout à coup");
    }

    #[test]
    fn malformed_substitutions_are_skipped() {
        assert_eq!(
            parse_substitutions("a=b|broken|=x|c="),
            vec![
                ("a".to_string(), "b".to_string()),
                ("c".to_string(), String::new())
            ]
        );
    }
}
