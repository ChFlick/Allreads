//! Saving and undoing the words a vocabulary check marks known (see
//! [`crate::vocab_check`]).

use rusqlite::params;

use super::term::Status;
use super::{Database, DbError};

/// The source marked words carry, shown as their badge and used to undo.
pub const VOCABULARY_CHECK_SOURCE: &str = "Vocabulary check";

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
        self.record_word_counts(Some(language_code))?;
        transaction.commit()?;
        Ok(added)
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
        self.record_word_counts(Some(language_code))?;
        transaction.commit()?;
        Ok(deleted)
    }

    /// How many words a vocabulary check marked in `language_code` that are
    /// still as it left them, for Settings ("Undo vocabulary check (1,820 words)").
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn vocabulary_check_count(&self, language_code: &str) -> Result<usize, DbError> {
        let count: i64 = self.conn().query_row(
            "SELECT COUNT(*) FROM term
             WHERE language_code = ?1 AND imported_from = ?2 AND status = ?3
               AND translation = '' AND notes = ''",
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

        assert_eq!(db.vocabulary_check_count("fr").unwrap(), 2);
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
    }
}
