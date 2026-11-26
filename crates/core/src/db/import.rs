//! Storing imported words: added, or upgraded when the import knows them better.

use rusqlite::{params, OptionalExtension};

use super::term::Status;
use super::{Database, DbError};

/// One word to import, already cleaned (see [`crate::import::clean_word`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportedWord {
    pub text: String,
    /// Its meaning, if the import has one; only fills an empty translation.
    pub meaning: Option<String>,
    pub status: Status,
}

/// What an import did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImportSummary {
    /// Words that were not saved before.
    pub added: u32,
    /// Saved words that the import knows better (e.g. level 2 → known).
    pub upgraded: u32,
    /// Saved words already as known as the import says, or ignored.
    pub unchanged: u32,
}

/// How well a status is known, for "the more known status wins". Ignored is
/// not on the scale: an ignored word stays ignored.
fn rank(status: Status) -> u8 {
    match status {
        Status::Learning(level) => level,
        Status::WellKnown => 6,
        Status::Ignored => 0,
    }
}

impl Database {
    /// Imports words in one transaction. A new word is saved with the import's
    /// status, meaning and `source` (the badge, e.g. "Anki · French
    /// vocabulary"). A saved word only changes if the import knows it better,
    /// and its meaning only fills an empty translation; nothing is ever
    /// downgraded or removed. With `card_elsewhere` (words from Anki), the
    /// words are never sent to AnkiDroid, so no duplicate card is made.
    ///
    /// # Errors
    /// Returns `NotFound` if the language is not stored, and a database error
    /// if a statement fails (then nothing is imported).
    pub fn import_words(
        &self,
        language_code: &str,
        source: &str,
        card_elsewhere: bool,
        words: &[ImportedWord],
    ) -> Result<ImportSummary, DbError> {
        if self.language(language_code)?.is_none() {
            return Err(DbError::NotFound {
                what: format!("language {language_code}"),
            });
        }
        let mut summary = ImportSummary::default();
        let transaction = self.conn().unchecked_transaction()?;
        for word in words {
            let display_text = word.text.trim();
            let Some(key) = self.term_key(language_code, display_text)? else {
                continue;
            };
            if key.is_empty() {
                continue;
            }
            let meaning = word.meaning.as_deref().map(str::trim).unwrap_or("");
            let existing: Option<(i64, i64, String)> = transaction
                .query_row(
                    "SELECT id, status, translation FROM term WHERE language_code = ?1 AND text = ?2",
                    params![language_code, key],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            let id = match existing {
                None => {
                    summary.added += 1;
                    transaction.query_row(
                        "INSERT INTO term (language_code, text, display_text, status, translation,
                             imported_from, card_elsewhere)
                         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7) RETURNING id",
                        params![
                            language_code,
                            key,
                            display_text,
                            word.status.code(),
                            meaning,
                            source,
                            card_elsewhere
                        ],
                        |row| row.get::<_, i64>(0),
                    )?
                }
                Some((id, code, translation)) => {
                    let current = u8::try_from(code)
                        .ok()
                        .and_then(Status::from_code)
                        .unwrap_or(word.status);
                    let better = current != Status::Ignored && rank(word.status) > rank(current);
                    let status = if better { word.status } else { current };
                    if better {
                        summary.upgraded += 1;
                    } else {
                        summary.unchanged += 1;
                    }
                    // A word from Anki has a card there: unless Allreads made one
                    // already, it must not make another.
                    let elsewhere = card_elsewhere && self.anki_link(id)?.is_none();
                    transaction.execute(
                        "UPDATE term SET status = ?2,
                             translation = CASE WHEN translation = '' THEN ?3 ELSE translation END,
                             card_elsewhere = MAX(card_elsewhere, ?4)
                         WHERE id = ?1",
                        params![id, status.code(), meaning, elsewhere],
                    )?;
                    // Only a word that changed may need its card updated.
                    let filled = translation.is_empty() && !meaning.is_empty();
                    if !better && !filled {
                        continue;
                    }
                    id
                }
            };
            let status: i64 =
                transaction.query_row("SELECT status FROM term WHERE id = ?1", [id], |row| {
                    row.get(0)
                })?;
            let status = u8::try_from(status)
                .ok()
                .and_then(Status::from_code)
                .unwrap_or(word.status);
            if self.wants_anki_card(id, status)? {
                self.queue_for_anki(id)?;
            }
        }
        self.record_word_counts(Some(language_code))?;
        transaction.commit()?;
        Ok(summary)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests_support::{french_db, new_term};

    fn word(text: &str, meaning: Option<&str>, status: Status) -> ImportedWord {
        ImportedWord {
            text: text.into(),
            meaning: meaning.map(str::to_string),
            status,
        }
    }

    #[test]
    fn new_words_are_added_with_their_badge_and_meaning() {
        let db = french_db();
        let summary = db
            .import_words(
                "fr",
                "Anki · Vocabulaire",
                true,
                &[word("Chat", Some("cat"), Status::WellKnown)],
            )
            .unwrap();
        assert_eq!(
            summary,
            ImportSummary {
                added: 1,
                upgraded: 0,
                unchanged: 0
            }
        );
        let chat = db.find_term("fr", "chat").unwrap().unwrap();
        assert_eq!(
            (
                chat.display_text.as_str(),
                chat.status,
                chat.translation.as_str(),
                chat.imported_from.as_str()
            ),
            ("Chat", Status::WellKnown, "cat", "Anki · Vocabulaire")
        );
    }

    #[test]
    fn the_more_known_status_wins_and_translations_are_kept() {
        let db = french_db();
        let mut chien = new_term("chien", Status::Learning(4));
        chien.translation = "dog".into();
        db.save_term(&chien).unwrap();
        db.save_term(&new_term("oiseau", Status::Learning(1)))
            .unwrap();
        db.save_term(&new_term("le", Status::Ignored)).unwrap();
        let summary = db
            .import_words(
                "fr",
                "Anki · A",
                true,
                &[
                    word("chien", Some("hound"), Status::Learning(2)),
                    word("oiseau", Some("bird"), Status::WellKnown),
                    word("le", None, Status::WellKnown),
                ],
            )
            .unwrap();
        assert_eq!(
            summary,
            ImportSummary {
                added: 0,
                upgraded: 1,
                unchanged: 2
            }
        );
        let chien = db.find_term("fr", "chien").unwrap().unwrap();
        assert_eq!(
            (chien.status, chien.translation.as_str()),
            (Status::Learning(4), "dog")
        );
        let oiseau = db.find_term("fr", "oiseau").unwrap().unwrap();
        assert_eq!(
            (oiseau.status, oiseau.translation.as_str()),
            (Status::WellKnown, "bird")
        );
        assert_eq!(
            db.find_term("fr", "le").unwrap().unwrap().status,
            Status::Ignored
        );
        // Words saved in Allreads keep no badge.
        assert_eq!(oiseau.imported_from, "");
    }

    #[test]
    fn words_from_anki_are_never_queued_but_others_are() {
        let db = french_db();
        db.import_words(
            "fr",
            "Anki · A",
            true,
            &[word("chat", None, Status::Learning(2))],
        )
        .unwrap();
        db.import_words(
            "fr",
            "CSV · mots.csv",
            false,
            &[word("chien", None, Status::Learning(2))],
        )
        .unwrap();
        let queued = |db: &Database| -> Vec<String> {
            db.anki_queue()
                .unwrap()
                .into_iter()
                .map(|q| db.term(q.term_id).unwrap().unwrap().text)
                .collect()
        };
        assert_eq!(queued(&db), ["chien"]);
        // Saving the Anki word again while reading still sends nothing.
        db.save_term(&new_term("chat", Status::Learning(3)))
            .unwrap();
        db.set_term_status("fr", "chat", Status::Learning(4))
            .unwrap();
        assert_eq!(queued(&db), ["chien"]);
    }

    #[test]
    fn an_unknown_language_imports_nothing() {
        let db = french_db();
        assert!(matches!(
            db.import_words("zz", "x", false, &[word("a", None, Status::WellKnown)]),
            Err(DbError::NotFound { .. })
        ));
    }
}
