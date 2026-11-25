//! Anki cards: the note type per language, and notes built from saved terms.
//!
//! Cards are sentence mining: the front shows the sentence the word was met
//! in, with the word in bold, and Anki reads it aloud with its `{{tts}}` tag.
//! The back has the word, its translation and dictionary form, the sentence's
//! translation, and the source. An optional second card goes from the word to
//! its meaning. Kotlin hands all of this to `AnkiDroid` unchanged.

use std::collections::HashMap;

use crate::db::term::Term;
use crate::db::{Database, DbError};
use crate::presets::LanguagePreset;
use crate::text::tokens::tokenize;
use crate::text::LanguageRules;

/// The note's fields, in order. Kotlin creates the note type with these, and
/// every note's `fields` follow the same order.
pub const FIELDS: [&str; 6] = [
    "Word",
    "Lemma",
    "Translation",
    "Sentence",
    "SentenceTranslation",
    "Source",
];

/// One card template of the note type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardTemplate {
    pub name: String,
    pub front: String,
    pub back: String,
}

/// The note type for one language, e.g. "Allreads (French)".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteType {
    pub name: String,
    pub fields: Vec<String>,
    pub cards: Vec<CardTemplate>,
    pub css: String,
}

/// The note type for `language`: a sentence card, plus a word card if asked.
/// The `{{tts}}` tag needs a fixed locale, which is why there is one note type
/// per language.
#[must_use]
pub fn note_type(language: &LanguagePreset, include_word_card: bool) -> NoteType {
    // Anki writes locales with an underscore: fr_FR, not fr-FR.
    let locale = language.tts_locale.replace('-', "_");
    let back_details = "{{#Lemma}}<div class=\"lemma\">{{Lemma}}</div>{{/Lemma}}\n\
         {{#SentenceTranslation}}<div class=\"sentence-translation\">{{SentenceTranslation}}</div>{{/SentenceTranslation}}\n\
         {{#Source}}<div class=\"source\">{{Source}}</div>{{/Source}}";
    let mut cards = vec![CardTemplate {
        name: "Sentence".into(),
        front: format!(
            "<div class=\"sentence\">{{{{Sentence}}}}</div>\n{{{{tts {locale}:Sentence}}}}"
        ),
        back: format!(
            "{{{{FrontSide}}}}\n<hr id=\"answer\">\n<div class=\"word\">{{{{Word}}}}</div>\n\
             <div class=\"translation\">{{{{Translation}}}}</div>\n{back_details}"
        ),
    }];
    if include_word_card {
        cards.push(CardTemplate {
            name: "Word".into(),
            front: format!("<div class=\"word\">{{{{Word}}}}</div>\n{{{{tts {locale}:Word}}}}"),
            back: format!(
                "{{{{FrontSide}}}}\n<hr id=\"answer\">\n<div class=\"translation\">{{{{Translation}}}}</div>\n\
                 <div class=\"sentence\">{{{{Sentence}}}}</div>\n{back_details}"
            ),
        });
    }
    NoteType {
        name: format!("Allreads ({})", language.name),
        fields: FIELDS.iter().map(|f| (*f).to_string()).collect(),
        cards,
        css: CSS.to_string(),
    }
}

const CSS: &str = ".card { font-family: Georgia, serif; font-size: 22px; line-height: 1.5; \
text-align: center; color: #1b1b1b; background: #fdfcf9; }
.sentence b { color: #b25e00; }
.word { font-size: 26px; font-weight: bold; margin-top: 8px; }
.translation { font-size: 20px; margin-top: 4px; }
.lemma { color: #666; font-size: 16px; }
.sentence-translation { color: #555; font-style: italic; margin-top: 12px; }
.source { color: #999; font-size: 13px; margin-top: 16px; }
.nightMode.card, .night_mode .card { color: #eee; background: #1e1e1e; }
.nightMode .sentence b, .night_mode .sentence b { color: #ffb74d; }
.nightMode .lemma, .nightMode .sentence-translation { color: #bbb; }
";

/// A note ready for `AnkiDroid`: add it when `note_id` is `None`, else update
/// that note's fields.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnkiNote {
    pub term_id: i64,
    pub language_code: String,
    pub note_id: Option<i64>,
    /// In [`FIELDS`] order; text is HTML-escaped.
    pub fields: Vec<String>,
    pub tags: Vec<String>,
}

/// Builds a term's note. The word is found in its sentence by tokenizing and
/// matching term keys, so case and apostrophe style do not matter. A word
/// saved without a sentence uses the word itself, so the card still exists.
#[must_use]
pub fn build_note(term: &Term, rules: &LanguageRules, note_id: Option<i64>) -> AnkiNote {
    let context = &term.context;
    let sentence = if context.sentence.trim().is_empty() {
        format!("<b>{}</b>", escape_html(&term.display_text))
    } else {
        bold_word(&context.sentence, &term.text, rules)
    };
    let source = match (context.source_book.trim(), context.source_chapter.trim()) {
        ("", _) => String::new(),
        (book, "") => book.to_string(),
        (book, chapter) => format!("{book} — {chapter}"),
    };
    let mut tags = vec!["allreads".to_string(), term.language_code.clone()];
    if let Some(tag) = book_tag(&context.source_book) {
        tags.push(tag);
    }
    AnkiNote {
        term_id: term.id,
        language_code: term.language_code.clone(),
        note_id,
        fields: vec![
            escape_html(&term.display_text),
            escape_html(term.lemma.as_deref().unwrap_or("")),
            escape_html(&term.translation),
            sentence,
            escape_html(&context.sentence_translation),
            escape_html(&source),
        ],
        tags,
    }
}

/// The sentence, HTML-escaped, with every token whose key is `key` in bold.
fn bold_word(sentence: &str, key: &str, rules: &LanguageRules) -> String {
    let mut html = String::with_capacity(sentence.len() + 7);
    for token in tokenize(sentence, rules) {
        let text = escape_html(&token.text);
        if token.key.as_deref() == Some(key) {
            html.push_str("<b>");
            html.push_str(&text);
            html.push_str("</b>");
        } else {
            html.push_str(&text);
        }
    }
    html
}

/// A tag for the book: lower case, words joined by `_`, at most 40 characters.
/// Anki tags cannot contain spaces.
fn book_tag(book: &str) -> Option<String> {
    let words: Vec<String> = book
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect();
    let tag: String = words.join("_").chars().take(40).collect();
    let tag = tag.trim_end_matches('_').to_string();
    (!tag.is_empty()).then_some(tag)
}

/// Escapes the characters HTML gives meaning to.
fn escape_html(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

/// The notes for every term waiting in the Anki queue, oldest first. A term
/// that has since been deleted is dropped from the queue.
///
/// # Errors
/// Returns a database error if a statement fails, and `InvalidValue` if a
/// language's rules do not compile.
pub fn pending_notes(db: &Database) -> Result<Vec<AnkiNote>, DbError> {
    let mut rules: HashMap<String, LanguageRules> = HashMap::new();
    let mut notes = Vec::new();
    for queued in db.anki_queue()? {
        let Some(term) = db.term(queued.term_id)? else {
            // Deleting a term also clears its queue entry, so this is only a safeguard.
            continue;
        };
        if !rules.contains_key(&term.language_code) {
            let language = db
                .language(&term.language_code)?
                .ok_or_else(|| DbError::NotFound {
                    what: format!("language {}", term.language_code),
                })?;
            let compiled = LanguageRules::new(&language).map_err(DbError::InvalidValue)?;
            rules.insert(term.language_code.clone(), compiled);
        }
        let note_id = db.anki_link(term.id)?.map(|link| link.note_id);
        notes.push(build_note(&term, &rules[&term.language_code], note_id));
    }
    Ok(notes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::term::{NewTerm, Status, TermContext};
    use crate::db::tests_support::french_db;
    use crate::text::test_rules::builtin;

    fn french_preset() -> LanguagePreset {
        crate::presets::builtin_presets()
            .unwrap()
            .into_iter()
            .find(|p| p.code == "fr")
            .unwrap()
    }

    /// A database with the built-in French preset, which splits `l’`.
    fn db() -> Database {
        let db = french_db();
        db.upsert_language(&french_preset()).unwrap();
        db
    }

    fn save(db: &Database, text: &str, status: Status, sentence: &str) -> i64 {
        db.save_term(&NewTerm {
            language_code: "fr".into(),
            text: text.into(),
            lemma: None,
            status,
            translation: "man".into(),
            notes: String::new(),
            parent_id: None,
            context: Some(TermContext {
                sentence: sentence.into(),
                sentence_translation: "The man stopped.".into(),
                source_book: "Le Comte de Monte-Cristo".into(),
                source_chapter: "Chapitre I".into(),
            }),
        })
        .unwrap()
    }

    #[test]
    fn the_note_type_has_six_fields_and_a_spoken_sentence_card() {
        let note_type = note_type(&french_preset(), false);
        assert_eq!(note_type.name, "Allreads (French)");
        assert_eq!(note_type.fields, FIELDS);
        assert_eq!(note_type.cards.len(), 1);
        assert!(note_type.cards[0].front.contains("{{Sentence}}"));
        assert!(note_type.cards[0].front.contains("{{tts fr_FR:Sentence}}"));
        assert!(note_type.cards[0].back.contains("{{FrontSide}}"));
    }

    #[test]
    fn the_word_card_is_optional() {
        let note_type = note_type(&french_preset(), true);
        let names: Vec<&str> = note_type.cards.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, ["Sentence", "Word"]);
        assert!(note_type.cards[1].front.contains("{{tts fr_FR:Word}}"));
    }

    #[test]
    fn a_note_bolds_the_word_whatever_its_case_or_apostrophe() {
        let db = db();
        let id = save(
            &db,
            "homme",
            Status::Learning(2),
            "L’Homme s’arrêta, puis un homme rit.",
        );
        let notes = pending_notes(&db).unwrap();
        assert_eq!(notes.len(), 1);
        let note = &notes[0];
        assert_eq!(note.term_id, id);
        assert_eq!(note.note_id, None);
        assert_eq!(
            note.fields[3],
            "L’<b>Homme</b> s’arrêta, puis un <b>homme</b> rit."
        );
        assert_eq!(note.fields[5], "Le Comte de Monte-Cristo — Chapitre I");
        assert_eq!(note.tags, ["allreads", "fr", "le_comte_de_monte_cristo"]);
    }

    #[test]
    fn note_snapshot() {
        let db = db();
        save(&db, "lit", Status::Learning(1), "Il lit un livre.");
        let note = pending_notes(&db).unwrap().remove(0);
        insta::assert_debug_snapshot!((note.fields, note.tags));
    }

    #[test]
    fn html_in_the_text_is_escaped() {
        let term = Term {
            id: 1,
            language_code: "fr".into(),
            text: "chat".into(),
            display_text: "chat".into(),
            lemma: Some("<chat>".into()),
            status: Status::Learning(1),
            translation: "cat & \"kitty\"".into(),
            notes: String::new(),
            parent_id: None,
            context: TermContext {
                sentence: "Le chat <i>dort</i>.".into(),
                ..TermContext::default()
            },
        };
        let note = build_note(&term, &builtin("fr"), Some(9));
        assert_eq!(note.fields[1], "&lt;chat&gt;");
        assert_eq!(note.fields[2], "cat &amp; &quot;kitty&quot;");
        assert_eq!(note.fields[3], "Le <b>chat</b> &lt;i&gt;dort&lt;/i&gt;.");
        assert_eq!(note.note_id, Some(9));
    }

    #[test]
    fn a_word_saved_without_a_sentence_still_gets_a_card() {
        let term = Term {
            id: 1,
            language_code: "fr".into(),
            text: "chat".into(),
            display_text: "Chat".into(),
            lemma: None,
            status: Status::Learning(1),
            translation: "cat".into(),
            notes: String::new(),
            parent_id: None,
            context: TermContext::default(),
        };
        let note = build_note(&term, &builtin("fr"), None);
        assert_eq!(note.fields[3], "<b>Chat</b>");
        assert_eq!(note.fields[5], "");
        assert_eq!(note.tags, ["allreads", "fr"]);
    }

    #[test]
    fn a_pushed_note_leaves_the_queue_and_is_updated_next_time() {
        let db = db();
        let id = save(&db, "homme", Status::Learning(2), "Un homme.");
        db.record_anki_push(id, 1001, 7).unwrap();
        assert!(pending_notes(&db).unwrap().is_empty());
        // Editing it queues it again, now as an update of note 1001.
        save(&db, "homme", Status::Learning(3), "Un homme.");
        assert_eq!(pending_notes(&db).unwrap()[0].note_id, Some(1001));
    }

    #[test]
    fn book_tags_are_tidy() {
        assert_eq!(
            book_tag("  L'Étranger (Camus) ").as_deref(),
            Some("l_étranger_camus")
        );
        assert_eq!(book_tag("???"), None);
        assert_eq!(book_tag(&"a ".repeat(40)).unwrap().len(), 39);
    }
}
