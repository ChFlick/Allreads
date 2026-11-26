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
    /// Only media tags: `[sound:…]`.
    sound: Regex,
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
        sound: Regex::new(r"\[(sound|image):[^\]]*\]").expect("valid pattern"),
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

/// A card's meaning field as plain text: HTML removed (line breaks become
/// "; "), media tags and cloze markers gone, spaces tidied. Unlike
/// [`clean_word`] it keeps everything else, e.g. "bed, layer (geology)".
#[must_use]
pub fn clean_meaning(raw: &str) -> Option<String> {
    let patterns = patterns();
    let text = patterns.cloze.replace_all(raw, "$1");
    let text = patterns.line_break.replace_all(&text, "\n");
    let text: String = Html::parse_fragment(&text).root_element().text().collect();
    let text = patterns.sound.replace_all(&text, " ");
    let parts: Vec<String> = text
        .split('\n')
        .map(|line| normalize_whitespace(&line.replace('\u{a0}', " ")))
        .filter(|line| !line.is_empty())
        .collect();
    let meaning = parts.join("; ");
    (!meaning.is_empty()).then_some(meaning)
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

/// Notes of one kind from an import source: an Anki note type, or the rows
/// of a text file. The user picks which field holds the word and the meaning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportSource {
    /// The note type's name, or the file's.
    pub name: String,
    /// Field (or column) names, in the order of each note's fields.
    pub fields: Vec<String>,
    pub notes: Vec<SourceNote>,
}

/// One note or row: its fields, and its review progress when the source has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceNote {
    pub fields: Vec<String>,
    pub progress: Option<CardProgress>,
}

/// Column names that mark a first row as a header in a plain CSV file.
const HEADER_WORDS: [&str; 16] = [
    "word",
    "front",
    "back",
    "meaning",
    "translation",
    "term",
    "definition",
    "expression",
    "vocabulary",
    "vocab",
    "english",
    "french",
    "german",
    "spanish",
    "sentence",
    "notes",
];

/// Reads a text export: Anki's "Notes in Plain Text" (with its `#separator:`,
/// `#html:` and `#columns:` header lines) or a plain CSV/TSV file. The
/// separator is the header's, else a tab, semicolon or comma, whichever the
/// first row has most of. Fields may be quoted with `"`, holding separators,
/// line breaks and `""` for a quote. Columns are named by `#columns:`, by a
/// first row that looks like a header ("Word,Meaning"), or "Column 1", "Column 2"…
#[must_use]
pub fn read_text_export(text: &str, name: &str) -> ImportSource {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut separator = None;
    let mut columns: Option<Vec<String>> = None;
    let mut body_start = 0;
    // Anki's header lines come first, each starting with '#'.
    for line in text.split_inclusive('\n') {
        let Some(setting) = line.trim_end().strip_prefix('#') else {
            break;
        };
        body_start += line.len();
        if let Some((key, value)) = setting.split_once(':') {
            match key.trim().to_lowercase().as_str() {
                "separator" => separator = separator_named(value.trim()),
                "columns" => {
                    columns = Some(
                        value
                            .trim_end_matches(['\r', '\n'])
                            .split(['\t', ',', ';'])
                            .map(|c| c.trim().to_string())
                            .collect(),
                    )
                }
                _ => {}
            }
        }
    }
    let body = &text[body_start..];
    let separator = separator.unwrap_or_else(|| guess_separator(body));
    let mut rows = split_rows(body, separator);
    let width = rows.iter().map(Vec::len).max().unwrap_or(0);
    let fields = match columns {
        Some(columns) => columns,
        None => {
            let looks_like_header = rows.first().is_some_and(|first| {
                first
                    .iter()
                    .any(|cell| HEADER_WORDS.contains(&cell.trim().to_lowercase().as_str()))
            });
            if looks_like_header {
                rows.remove(0)
            } else {
                (1..=width).map(|n| format!("Column {n}")).collect()
            }
        }
    };
    ImportSource {
        name: name.to_string(),
        notes: rows
            .into_iter()
            .map(|mut cells| {
                cells.resize(fields.len().max(cells.len()), String::new());
                SourceNote {
                    fields: cells,
                    progress: None,
                }
            })
            .collect(),
        fields,
    }
}

/// The separator an Anki header names: `tab`, `comma`, `semicolon`, `space`,
/// `pipe`, `colon`, or the character itself.
fn separator_named(name: &str) -> Option<char> {
    match name.to_lowercase().as_str() {
        "tab" => Some('\t'),
        "comma" => Some(','),
        "semicolon" => Some(';'),
        "space" => Some(' '),
        "pipe" => Some('|'),
        "colon" => Some(':'),
        other => other.chars().next(),
    }
}

/// Tab, semicolon or comma: whichever the first line has most of (tab on a tie).
fn guess_separator(body: &str) -> char {
    let first = body.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let count = |c| first.matches(c).count();
    let (tabs, semicolons, commas) = (count('\t'), count(';'), count(','));
    if tabs >= semicolons && tabs >= commas && tabs > 0 {
        '\t'
    } else if semicolons > commas {
        ';'
    } else {
        ','
    }
}

/// Splits CSV-style text into rows of cells, honouring quotes. Empty lines are skipped.
fn split_rows(body: &str, separator: char) -> Vec<Vec<String>> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = body.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cell.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                cell.push(c);
            }
        } else if c == '"' && cell.is_empty() {
            quoted = true;
        } else if c == separator {
            row.push(std::mem::take(&mut cell));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut cell));
            if row.iter().any(|c| !c.trim().is_empty()) {
                rows.push(std::mem::take(&mut row));
            } else {
                row.clear();
            }
        } else {
            cell.push(c);
        }
    }
    row.push(cell);
    if row.iter().any(|c| !c.trim().is_empty()) {
        rows.push(row);
    }
    rows
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
    fn meanings_keep_their_text_without_markup() {
        assert_eq!(
            clean_meaning("bed, layer <i>(geology)</i>[sound:lit.mp3]").as_deref(),
            Some("bed, layer (geology)")
        );
        assert_eq!(
            clean_meaning("<div>bed</div><div>layer</div>").as_deref(),
            Some("bed; layer")
        );
        assert_eq!(clean_meaning(" [sound:x.mp3] "), None);
    }

    #[test]
    fn an_anki_plain_text_export_is_read_with_its_header() {
        let export = "#separator:tab\n#html:true\n#columns:Front\tBack\tTags\nle chat\t\"cat\nkitty\"\tanimals\nchien\tdog\t\n";
        let source = read_text_export(export, "deck.txt");
        assert_eq!(source.fields, ["Front", "Back", "Tags"]);
        assert_eq!(source.notes.len(), 2);
        assert_eq!(source.notes[0].fields, ["le chat", "cat\nkitty", "animals"]);
        assert_eq!(source.notes[1].fields, ["chien", "dog", ""]);
        assert_eq!(source.notes[0].progress, None);
    }

    #[test]
    fn a_plain_csv_with_or_without_a_header_row() {
        let with_header = read_text_export(
            "Word,Meaning\r\n\"maison, la\",house\r\n\r\n\"dit \"\"oui\"\"\",said\r\n",
            "words.csv",
        );
        assert_eq!(with_header.fields, ["Word", "Meaning"]);
        assert_eq!(with_header.notes[0].fields, ["maison, la", "house"]);
        assert_eq!(with_header.notes[1].fields, ["dit \"oui\"", "said"]);
        let bare = read_text_export("\u{feff}chat;cat\nchien;dog;extra\n", "mots.csv");
        assert_eq!(bare.fields, ["Column 1", "Column 2", "Column 3"]);
        assert_eq!(bare.notes[0].fields, ["chat", "cat", ""]);
        assert_eq!(bare.notes[1].fields, ["chien", "dog", "extra"]);
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
