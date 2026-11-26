//! Importing words studied elsewhere (Anki decks, text exports): turning a
//! card's field into the bare word, and a card's review progress into a status.

use std::sync::OnceLock;

use regex::Regex;
use scraper::Html;

use crate::db::term::Status;
use crate::text::normalize_whitespace;

/// Patterns used by [`clean_word`], compiled once.
struct Patterns {
    /// `<br>`, `<div>` and `<p>` start a new line: only the first line is the word.
    line_break: Regex,
    /// `[sound:…]`, `[image:…]` and other bracketed notes, such as `[ʃa]`.
    square: Regex,
    /// `(m.)`, `(fam.)` and other notes in round brackets.
    round: Regex,
    /// Cloze deletions: `{{c1::chat::animal}}` → `chat`.
    cloze: Regex,
}

fn patterns() -> &'static Patterns {
    static PATTERNS: OnceLock<Patterns> = OnceLock::new();
    PATTERNS.get_or_init(|| Patterns {
        line_break: Regex::new(r"(?i)<\s*(br|div|p)\b[^>]*>").expect("valid pattern"),
        square: Regex::new(r"\[[^\]]*\]").expect("valid pattern"),
        round: Regex::new(r"\([^)]*\)").expect("valid pattern"),
        cloze: Regex::new(r"\{\{c\d+::(.*?)(?:::.*?)?\}\}").expect("valid pattern"),
    })
}

/// The bare word in a card's field, or `None` if nothing is left.
///
/// In order: cloze deletions become their answer; HTML is removed (a line
/// break ends the word) and entities decoded; `[sound:…]` and other notes
/// in square or round brackets go; only the first part with letters before
/// a `,` `;` `/` `|` or line break stays; outer punctuation and spaces are trimmed;
/// then one leading article from `articles` (pipe-separated, e.g.
/// `le|la|les|l'`) is dropped, so "le chat (m.)" becomes "chat". An article
/// that is the whole word stays ("la" alone is a word too).
#[must_use]
pub fn clean_word(raw: &str, articles: &str) -> Option<String> {
    let patterns = patterns();
    let text = patterns.cloze.replace_all(raw, "$1");
    let text = patterns.line_break.replace_all(&text, "\n");
    // Parsing as an HTML fragment removes tags and decodes `&amp;`, `&nbsp;` …
    let text: String = Html::parse_fragment(&text).root_element().text().collect();
    let text = patterns.square.replace_all(&text, " ");
    let text = patterns.round.replace_all(&text, " ");
    // The first part with letters: a field may open with an empty line (`<div>`).
    let first = text
        .split([',', ';', '/', '|', '\n'])
        .find(|part| part.chars().any(char::is_alphabetic))
        .unwrap_or("");
    // Typographic apostrophes as in the presets' articles ("l’" → "l'").
    let first = first.replace(['’', '‘'], "'");
    let word = normalize_whitespace(&first.replace('\u{a0}', " "));
    let word = word.trim_matches(|c: char| c.is_whitespace() || ".!?:\"«»“”„".contains(c));
    let word = strip_article(word, articles);
    word.chars()
        .any(char::is_alphabetic)
        .then(|| word.to_string())
}

/// `word` without one leading article: "le chat" → "chat", "l'homme" → "homme".
fn strip_article<'a>(word: &'a str, articles: &str) -> &'a str {
    let lower = word.to_lowercase();
    for article in articles.split('|').map(str::trim).filter(|a| !a.is_empty()) {
        let article = article.to_lowercase();
        // An elided article ("l'") joins its word; others are followed by a space.
        let rest = if article.ends_with('\'') {
            lower.strip_prefix(article.as_str())
        } else {
            lower
                .strip_prefix(article.as_str())
                .and_then(|r| r.strip_prefix(' '))
        };
        if let Some(rest) = rest {
            let rest = rest.trim_start();
            if !rest.is_empty() {
                // Same suffix in the original, so its capitals are kept.
                return word[word.len() - rest.len()..].trim_start();
            }
        }
    }
    word
}

/// Where a card stands in Anki's reviews.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardProgress {
    /// Not studied yet, or still in its first learning steps.
    New,
    /// In review, with this interval in days.
    Interval(u32),
    /// Suspended by the user.
    Suspended,
}

/// The status a card's progress gives a word when an import follows Anki's
/// reviews: new or learning → level 1; an interval under 3 days → 2, under 7
/// → 3, under 14 → 4, under 21 → 5; 21 days or more (a mature card) → known.
/// Suspended cards give no status: they are skipped.
#[must_use]
pub fn status_for(progress: CardProgress) -> Option<Status> {
    match progress {
        CardProgress::Suspended => None,
        CardProgress::New => Some(Status::Learning(1)),
        CardProgress::Interval(days) => Some(match days {
            0..=2 => Status::Learning(2),
            3..=6 => Status::Learning(3),
            7..=13 => Status::Learning(4),
            14..=20 => Status::Learning(5),
            _ => Status::WellKnown,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRENCH: &str = "le|la|les|l'|un|une|des|du|de la";
    const GERMAN: &str = "der|die|das|den|dem|des|ein|eine|einen|einem|einer|eines";
    const SPANISH: &str = "el|la|los|las|un|una|unos|unas|lo";
    const ENGLISH: &str = "the|a|an|to";

    fn clean(raw: &str, articles: &str) -> Option<String> {
        clean_word(raw, articles)
    }

    #[test]
    fn html_media_and_cloze_are_removed() {
        assert_eq!(clean("<b>chat</b>&nbsp;", FRENCH).as_deref(), Some("chat"));
        assert_eq!(
            clean("chat[sound:chat_fr.mp3]", FRENCH).as_deref(),
            Some("chat")
        );
        assert_eq!(
            clean("<div>pierre</div><div>stone</div>", FRENCH).as_deref(),
            Some("pierre")
        );
        assert_eq!(clean("chat<br>cat", FRENCH).as_deref(), Some("chat"));
        assert_eq!(clean("{{c1::lire::verb}}", FRENCH).as_deref(), Some("lire"));
        assert_eq!(
            clean("Tom &amp; Jerry", FRENCH).as_deref(),
            Some("Tom & Jerry")
        );
    }

    #[test]
    fn notes_and_alternatives_are_dropped() {
        assert_eq!(clean("chat (m.)", FRENCH).as_deref(), Some("chat"));
        assert_eq!(clean("chat [ʃa]", FRENCH).as_deref(), Some("chat"));
        assert_eq!(clean("manger, mangé", FRENCH).as_deref(), Some("manger"));
        assert_eq!(clean("beau / belle", FRENCH).as_deref(), Some("beau"));
        assert_eq!(
            clean("partir; s'en aller", FRENCH).as_deref(),
            Some("partir")
        );
        assert_eq!(
            clean("  « bonjour ! »  ", FRENCH).as_deref(),
            Some("bonjour")
        );
    }

    #[test]
    fn a_leading_article_is_dropped() {
        assert_eq!(clean("le chat", FRENCH).as_deref(), Some("chat"));
        assert_eq!(clean("La Maison", FRENCH).as_deref(), Some("Maison"));
        assert_eq!(clean("l'homme", FRENCH).as_deref(), Some("homme"));
        assert_eq!(clean("l’école", FRENCH).as_deref(), Some("école"));
        assert_eq!(clean("de la farine", FRENCH).as_deref(), Some("farine"));
        assert_eq!(clean("das Haus", GERMAN).as_deref(), Some("Haus"));
        assert_eq!(clean("los libros", SPANISH).as_deref(), Some("libros"));
        assert_eq!(clean("to read", ENGLISH).as_deref(), Some("read"));
        // Only one article, and only a whole one: "leçon" does not start with "le ".
        assert_eq!(clean("leçon", FRENCH).as_deref(), Some("leçon"));
        assert_eq!(clean("la la la", FRENCH).as_deref(), Some("la la"));
        // An article alone is a word.
        assert_eq!(clean("la", FRENCH).as_deref(), Some("la"));
        // Phrases stay phrases.
        assert_eq!(
            clean("pierre précieuse", FRENCH).as_deref(),
            Some("pierre précieuse")
        );
    }

    #[test]
    fn nothing_left_is_none() {
        assert_eq!(clean("", FRENCH), None);
        assert_eq!(clean("[sound:x.mp3]", FRENCH), None);
        assert_eq!(clean("<img src=\"a.jpg\">", FRENCH), None);
        assert_eq!(clean("(m.) 42", FRENCH), None);
    }

    #[test]
    fn review_progress_maps_to_levels_at_each_boundary() {
        assert_eq!(status_for(CardProgress::New), Some(Status::Learning(1)));
        assert_eq!(status_for(CardProgress::Suspended), None);
        let at = |days| status_for(CardProgress::Interval(days));
        assert_eq!(at(0), Some(Status::Learning(2)));
        assert_eq!(at(2), Some(Status::Learning(2)));
        assert_eq!(at(3), Some(Status::Learning(3)));
        assert_eq!(at(6), Some(Status::Learning(3)));
        assert_eq!(at(7), Some(Status::Learning(4)));
        assert_eq!(at(13), Some(Status::Learning(4)));
        assert_eq!(at(14), Some(Status::Learning(5)));
        assert_eq!(at(20), Some(Status::Learning(5)));
        assert_eq!(at(21), Some(Status::WellKnown));
        assert_eq!(at(400), Some(Status::WellKnown));
    }
}
