//! Saving and undoing the words a vocabulary check marks known (see
//! [`crate::vocab_check`]).

use rusqlite::{params, OptionalExtension};

use super::term::Status;
use super::{Database, DbError};

/// The source marked words carry, shown as their badge and used to undo.
pub const VOCABULARY_CHECK_SOURCE: &str = "Vocabulary check";

/// How a language's vocabulary checks have held up while reading.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VocabCheckStats {
    /// Word forms the checks marked known.
    pub marked: usize,
    /// Of those, how many the reader set back (another level, or forgotten):
    /// the marks that were wrong, as far as the app can tell.
    pub corrected: usize,
}

fn marked_key(language_code: &str) -> String {
    format!("vocab_check_marked:{language_code}")
}

fn corrected_key(language_code: &str) -> String {
    format!("vocab_check_corrected:{language_code}")
}

impl Database {
    /// Saves `keys` (term keys) as known in `language_code`, with the source
    /// [`VOCABULARY_CHECK_SOURCE`]. Words already saved are left exactly as
    /// they are, so a word being learned keeps its level. Known words get no
    /// Anki cards. Returns how many words were added.
    ///
    /// # Errors
    /// Returns `NotFound` for an unknown language and a database error if a
    /// statement fails (then nothing is saved).
    pub fn mark_known_from_check(
        &self,
        language_code: &str,
        keys: &[String],
    ) -> Result<usize, DbError> {
        if self.language(language_code)?.is_none() {
            return Err(DbError::NotFound {
                what: format!("language {language_code}"),
            });
        }
        let transaction = self.conn().unchecked_transaction()?;
        let mut added = 0;
        {
            let mut insert = transaction.prepare(
                "INSERT INTO term (language_code, text, display_text, status, imported_from)
                 VALUES (?1, ?2, ?2, ?3, ?4)
                 ON CONFLICT (language_code, text) DO NOTHING",
            )?;
            for key in keys.iter().filter(|key| !key.trim().is_empty()) {
                added += insert.execute(params![
                    language_code,
                    key,
                    Status::WellKnown.code(),
                    VOCABULARY_CHECK_SOURCE
                ])?;
            }
        }
        self.add_to_counter(&marked_key(language_code), added)?;
        self.record_word_counts(Some(language_code))?;
        transaction.commit()?;
        Ok(added)
    }

    /// How the checks in `language_code` have held up: words marked, and how
    /// many of them the reader set back since.
    ///
    /// # Errors
    /// Returns a database error if a setting cannot be read.
    pub fn vocabulary_check_stats(&self, language_code: &str) -> Result<VocabCheckStats, DbError> {
        Ok(VocabCheckStats {
            marked: self.counter(&marked_key(language_code))?,
            corrected: self.counter(&corrected_key(language_code))?,
        })
    }

    /// Called before term `id` changes to `status` (`None`: deleted). A word
    /// the check marked known that leaves "known" counts as a correction, and
    /// becomes the reader's own word: no longer from the check, so undo and
    /// the badge leave it alone.
    pub(crate) fn note_vocabulary_check_change(
        &self,
        id: i64,
        status: Option<Status>,
    ) -> Result<(), DbError> {
        let found: Option<(String, String, i64)> = self
            .conn()
            .query_row(
                "SELECT language_code, imported_from, status FROM term WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((language_code, source, code)) = found else {
            return Ok(());
        };
        let marked_known =
            source == VOCABULARY_CHECK_SOURCE && code == i64::from(Status::WellKnown.code());
        if !marked_known || status == Some(Status::WellKnown) {
            return Ok(());
        }
        self.add_to_counter(&corrected_key(&language_code), 1)?;
        if status.is_some() {
            self.conn()
                .execute("UPDATE term SET imported_from = '' WHERE id = ?1", [id])?;
        }
        Ok(())
    }

    fn counter(&self, key: &str) -> Result<usize, DbError> {
        Ok(self.setting(key)?.and_then(|v| v.parse().ok()).unwrap_or(0))
    }

    fn add_to_counter(&self, key: &str, amount: usize) -> Result<(), DbError> {
        let value = self.counter(key)? + amount;
        self.set_setting(key, &value.to_string())
    }

    /// Takes back a vocabulary check in `language_code`: deletes the words it
    /// marked that are still known and untouched (no meaning, notes or Anki
    /// card added since). Returns how many were deleted.
    ///
    /// # Errors
    /// Returns a database error if a statement fails.
    pub fn undo_vocabulary_check(&self, language_code: &str) -> Result<usize, DbError> {
        let transaction = self.conn().unchecked_transaction()?;
        let deleted = transaction.execute(
            "DELETE FROM term
             WHERE language_code = ?1 AND imported_from = ?2 AND status = ?3
               AND translation = '' AND notes = ''
               AND id NOT IN (SELECT term_id FROM anki_link)",
            params![
                language_code,
                VOCABULARY_CHECK_SOURCE,
                Status::WellKnown.code()
            ],
        )?;
        // Starting over: the counts are about marks that no longer exist.
        self.set_setting(&marked_key(language_code), "0")?;
        self.set_setting(&corrected_key(language_code), "0")?;
        self.record_word_counts(Some(language_code))?;
        transaction.commit()?;
        Ok(deleted)
    }

    /// How many words a vocabulary check marked in `language_code` that are
    /// still as it left them (the ones [`Database::undo_vocabulary_check`]
    /// would delete), for Settings ("Undo vocabulary check (1,820 words)").
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn vocabulary_check_count(&self, language_code: &str) -> Result<usize, DbError> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM term
             WHERE language_code = ?1 AND imported_from = ?2 AND status = ?3
               AND translation = '' AND notes = ''
               AND id NOT IN (SELECT term_id FROM anki_link)",
            params![
                language_code,
                VOCABULARY_CHECK_SOURCE,
                Status::WellKnown.code()
            ],
            |row| row.get(0),
        )?;
        Ok(usize::try_from(count).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests_support::{french_db, new_term};

    fn keys(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| (*w).to_string()).collect()
    }

    #[test]
    fn marked_words_are_known_and_saved_words_keep_their_level() {
        let db = french_db();
        db.save_term(&new_term("chat", Status::Learning(2)))
            .unwrap();
        let added = db
            .mark_known_from_check(
                "fr",
                &keys(&["le", "la", "chat", "lire", "lisait", "", "le"]),
            )
            .unwrap();
        assert_eq!(added, 4);
        let status = |key: &str| db.find_term("fr", key).unwrap().map(|t| t.status);
        assert_eq!(status("lisait"), Some(Status::WellKnown));
        assert_eq!(status("chat"), Some(Status::Learning(2)));
        let le = db.find_term("fr", "le").unwrap().unwrap();
        assert_eq!(le.imported_from, VOCABULARY_CHECK_SOURCE);
        assert_eq!(db.vocabulary_check_count("fr").unwrap(), 4);
        // Known words are not sent to AnkiDroid.
        assert!(
            db.waiting_for_anki("fr").unwrap().len() <= 1,
            "only chat may wait"
        );
        assert!(matches!(
            db.mark_known_from_check("xx", &keys(&["a"])),
            Err(DbError::NotFound { .. })
        ));
    }

    #[test]
    fn undo_removes_only_the_untouched_marked_words() {
        let db = french_db();
        db.save_term(&new_term("chat", Status::Learning(2)))
            .unwrap();
        db.mark_known_from_check("fr", &keys(&["le", "la", "lire", "chien"]))
            .unwrap();
        // Since then: "lire" was set back to learning, "la" got a note, "chien" a card.
        let lire = db.find_term("fr", "lire").unwrap().unwrap();
        db.set_term_status("fr", "lire", Status::Learning(1))
            .unwrap();
        let mut la = new_term("la", Status::WellKnown);
        la.notes = "article".into();
        db.save_term(&la).unwrap();
        let chien = db.find_term("fr", "chien").unwrap().unwrap().id;
        db.link_anki_note(chien, 1001, 77).unwrap();

        assert_eq!(db.vocabulary_check_count("fr").unwrap(), 1);
        assert_eq!(db.undo_vocabulary_check("fr").unwrap(), 1);
        assert!(db.find_term("fr", "le").unwrap().is_none());
        assert_eq!(db.find_term("fr", "lire").unwrap().unwrap().id, lire.id);
        assert!(db.find_term("fr", "la").unwrap().is_some());
        assert!(db.find_term("fr", "chien").unwrap().is_some());
        assert_eq!(
            db.find_term("fr", "chat").unwrap().unwrap().status,
            Status::Learning(2)
        );
        assert_eq!(db.undo_vocabulary_check("fr").unwrap(), 0);
        assert_eq!(db.vocabulary_check_count("fr").unwrap(), 0);
    }

    #[test]
    fn setting_a_marked_word_back_counts_as_a_correction() {
        let db = french_db();
        db.mark_known_from_check("fr", &keys(&["le", "la", "lire", "chien", "chat"]))
            .unwrap();
        assert_eq!(
            db.vocabulary_check_stats("fr").unwrap(),
            VocabCheckStats {
                marked: 5,
                corrected: 0
            }
        );
        // Not corrections: a meaning added while still known.
        let mut le = new_term("le", Status::WellKnown);
        le.translation = "the".into();
        db.save_term(&le).unwrap();
        // Corrections: a level from the drawer, a double tap's level, and a forget.
        db.save_term(&new_term("lire", Status::Learning(2)))
            .unwrap();
        db.set_term_status("fr", "chien", Status::Learning(1))
            .unwrap();
        let chat = db.find_term("fr", "chat").unwrap().unwrap().id;
        db.delete_term(chat).unwrap();
        assert_eq!(
            db.vocabulary_check_stats("fr").unwrap(),
            VocabCheckStats {
                marked: 5,
                corrected: 3
            }
        );
        // A corrected word is the reader's own now: no badge, and undo leaves it.
        assert_eq!(
            db.find_term("fr", "lire").unwrap().unwrap().imported_from,
            ""
        );
        db.set_term_status("fr", "lire", Status::WellKnown).unwrap();
        assert_eq!(db.vocabulary_check_stats("fr").unwrap().corrected, 3);
        db.undo_vocabulary_check("fr").unwrap();
        assert!(db.find_term("fr", "lire").unwrap().is_some());
        assert!(db.find_term("fr", "la").unwrap().is_none());
        assert_eq!(
            db.vocabulary_check_stats("fr").unwrap(),
            VocabCheckStats::default()
        );
    }
}
