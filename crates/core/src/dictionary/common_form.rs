//! The form a word is saved and translated in: "Coq" at the start of a
//! sentence is the common word "coq", while "Pierre" the name stays "Pierre".

use super::WordKinds;
use crate::text::capitalises_nouns;

/// The form of `text` to save and to send to a translator.
///
/// A capitalised word is lower-cased when the dictionary (`kinds`) knows it as
/// a common word, so "Coq" at a sentence start becomes "coq" and DeepL does
/// not read it as a name. It stays as written when:
/// - the language capitalises every noun (German "Haus" stays "Haus"),
/// - the dictionary knows it only as a name ("Paris"), or does not know it,
/// - there is no dictionary (`kinds` is `None`).
#[must_use]
pub fn common_form(text: &str, kinds: Option<&WordKinds>, language_code: &str) -> String {
    let lower = text.to_lowercase();
    let lower_it = lower != text
        && !capitalises_nouns(language_code)
        && kinds.is_some_and(WordKinds::is_common);
    if lower_it {
        lower
    } else {
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(parts: &[&str], lemmas: &[&str]) -> WordKinds {
        WordKinds {
            parts_of_speech: parts.iter().map(|p| (*p).to_string()).collect(),
            lemmas: lemmas.iter().map(|l| (*l).to_string()).collect(),
        }
    }

    #[test]
    fn a_capitalised_common_word_is_lower_cased() {
        assert_eq!(
            common_form("Coq", Some(&kinds(&["noun"], &[])), "fr"),
            "coq"
        );
        // A name that is also a common word: "Pierre" / "pierre" (stone).
        assert_eq!(
            common_form("Pierre", Some(&kinds(&["name", "noun"], &[])), "fr"),
            "pierre"
        );
        // A form of a common word.
        assert_eq!(
            common_form("Lisait", Some(&kinds(&[], &["lire"])), "fr"),
            "lisait"
        );
        assert_eq!(
            common_form("Pierre précieuse", Some(&kinds(&["noun"], &[])), "fr"),
            "pierre précieuse"
        );
    }

    #[test]
    fn names_unknown_words_and_german_nouns_stay_as_written() {
        assert_eq!(
            common_form("Paris", Some(&kinds(&["name"], &[])), "fr"),
            "Paris"
        );
        assert_eq!(
            common_form("Gregor", Some(&WordKinds::default()), "de"),
            "Gregor"
        );
        assert_eq!(common_form("Coq", None, "fr"), "Coq");
        assert_eq!(
            common_form("Haus", Some(&kinds(&["noun"], &[])), "de"),
            "Haus"
        );
        assert_eq!(
            common_form("chat", Some(&kinds(&["noun"], &[])), "fr"),
            "chat"
        );
    }
}
