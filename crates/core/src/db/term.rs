//! Saved vocabulary terms and their statuses.

use rusqlite::{params, OptionalExtension, Row};

use super::{require_text, Database, DbError};
use crate::text::key::{parse_substitutions, term_key};

/// A word's status. A word with no saved term is "new".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Being learned, at level 1 to 5.
    Learning(u8),
    /// Status 98: never highlight or review.
    Ignored,
    /// Status 99: known well enough to need no card.
    WellKnown,
}

impl Status {
    /// A learning status at `level`, or `None` if the level is not 1 to 5.
    #[must_use]
    pub fn learning(level: u8) -> Option<Self> {
        (1..=5).contains(&level).then_some(Self::Learning(level))
    }

    /// The number stored in the database.
    #[must_use]
    pub fn code(self) -> u8 {
        match self {
            Self::Learning(level) => level,
            Self::Ignored => 98,
            Self::WellKnown => 99,
        }
    }

    /// True for the statuses that become Anki cards: learning levels 1 to 4.
    /// Level 5, known and ignored words do not.
    #[must_use]
    pub fn makes_anki_card(self) -> bool {
        matches!(self, Self::Learning(1..=4))
    }

    /// The status for a stored number, or `None` if the number is not a status.
    #[must_use]
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            98 => Some(Self::Ignored),
            99 => Some(Self::WellKnown),
            _ => Self::learning(code),
        }
    }
}

/// A term to save. Saving a word whose term key is already stored in this
/// language updates that term, so "Chat" and "chat" are one term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTerm {
    pub language_code: String,
    /// The word or phrase as the reader saw it.
    pub text: String,
    pub lemma: Option<String>,
    pub status: Status,
    pub translation: String,
    pub notes: String,
    /// The lemma this form links to. `None` keeps an existing link, so
    /// re-saving a word from the reader never unlinks it.
    pub parent_id: Option<i64>,
    /// Where the word was met, for its Anki card. `None` keeps the stored
    /// context, so re-saving from the Words tab never erases the sentence.
    pub context: Option<TermContext>,
}

/// Where a word was met: its sentence and source, for the Anki card.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TermContext {
    pub sentence: String,
    /// The sentence's translation, if one was shown while reading.
    pub sentence_translation: String,
    pub source_book: String,
    pub source_chapter: String,
}

/// A saved term.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    pub id: i64,
    pub language_code: String,
    /// The term key (lower case, normalised), used for matching.
    pub text: String,
    /// The form as first seen, for display and Anki cards (keeps "Haus" capitalised).
    pub display_text: String,
    pub lemma: Option<String>,
    pub status: Status,
    pub translation: String,
    pub notes: String,
    pub parent_id: Option<i64>,
    /// Where the word was saved; empty fields when it was saved without context.
    pub context: TermContext,
    /// Where an imported word came from ("Anki · French vocabulary"); empty otherwise.
    pub imported_from: String,
}

/// The current time in Unix milliseconds, in SQL (as for `anki_suspend.queued_at`).
const NOW_MS: &str = "CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER)";

const COLUMNS: &str = "id, language_code, text, lemma, status, translation, notes, parent_id, \
     display_text, context_sentence, context_translation, source_book, source_chapter, imported_from";

impl Database {
    /// Saves a term and returns its id. An existing term with the same language
    /// and text keeps its id and is updated.
    ///
    /// # Errors
    /// Returns `InvalidValue` for blank text or a bad level, `NotFound` if the
    /// language is not stored, and a database error if the parent term does not exist.
    pub fn save_term(&self, term: &NewTerm) -> Result<i64, DbError> {
        let display_text = term.text.trim();
        require_text("term text", display_text)?;
        let key = self
            .term_key(&term.language_code, display_text)?
            .ok_or_else(|| DbError::NotFound {
                what: format!("language {}", term.language_code),
            })?;
        if let Status::Learning(level) = term.status {
            if !(1..=5).contains(&level) {
                return Err(DbError::InvalidValue(format!(
                    "learning level {level} is not 1 to 5"
                )));
            }
        }
        self.check_parent_link(&term.language_code, display_text, term.parent_id)?;
        // On conflict the display text is kept: it is the form as first seen.
        // The context (?9..?12) is NULL when not given, which keeps the stored one.
        let context = term.context.as_ref();
        // The term and its queue entry are saved together or not at all.
        let transaction = self.conn().unchecked_transaction()?;
        let id = transaction.query_row(
            "INSERT INTO term (language_code, text, lemma, status, translation, notes, parent_id,
                 display_text, context_sentence, context_translation, source_book, source_chapter)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                 COALESCE(?9, ''), COALESCE(?10, ''), COALESCE(?11, ''), COALESCE(?12, ''))
             ON CONFLICT (language_code, text) DO UPDATE SET
                 lemma = excluded.lemma,
                 status = excluded.status,
                 translation = excluded.translation,
                 notes = excluded.notes,
                 parent_id = COALESCE(excluded.parent_id, term.parent_id),
                 context_sentence = COALESCE(?9, term.context_sentence),
                 context_translation = COALESCE(?10, term.context_translation),
                 source_book = COALESCE(?11, term.source_book),
                 source_chapter = COALESCE(?12, term.source_chapter)
             RETURNING id",
            params![
                term.language_code,
                key,
                term.lemma,
                term.status.code(),
                term.translation,
                term.notes,
                term.parent_id,
                display_text,
                context.map(|c| c.sentence.trim()),
                context.map(|c| c.sentence_translation.trim()),
                context.map(|c| c.source_book.trim()),
                context.map(|c| c.source_chapter.trim()),
            ],
            |row| row.get(0),
        )?;
        if self.wants_anki_card(id, term.status)? {
            self.queue_for_anki(id)?;
        }
        // Saved again (e.g. Undo after forgetting): its card is not suspended after all.
        self.conn().execute(
            "DELETE FROM anki_suspend WHERE language_code = ?1 AND term_key = ?2",
            params![term.language_code, key],
        )?;
        // In the same transaction, so a save is one commit.
        self.record_word_counts(Some(&term.language_code))?;
        transaction.commit()?;
        Ok(id)
    }

    /// Whether a save at `status` should go to AnkiDroid: words being learned
    /// (levels 1 to 4) get a card, and a word with a card has it updated. A
    /// word with a card elsewhere (imported from Anki) never does, so no
    /// duplicate is made.
    pub(crate) fn wants_anki_card(&self, id: i64, status: Status) -> Result<bool, DbError> {
        let elsewhere: bool = self.conn().query_row(
            "SELECT card_elsewhere FROM term WHERE id = ?1",
            [id],
            |row| row.get(0),
        )?;
        Ok(!elsewhere && (status.makes_anki_card() || self.anki_link(id)?.is_some()))
    }

    /// Fails if `parent_id` is the term itself, or if following its parents
    /// would come back to the term. Following a loop would never end.
    fn check_parent_link(
        &self,
        language_code: &str,
        text: &str,
        parent_id: Option<i64>,
    ) -> Result<(), DbError> {
        let own_id = self.find_term(language_code, text)?.map(|term| term.id);
        let mut current = parent_id;
        let mut seen = std::collections::HashSet::new();
        while let Some(id) = current {
            if Some(id) == own_id {
                return Err(DbError::InvalidValue(
                    "this lemma link would form a loop".into(),
                ));
            }
            // A loop that already exists in the data must not hang us either.
            if !seen.insert(id) {
                break;
            }
            current = self.term(id)?.and_then(|term| term.parent_id);
        }
        Ok(())
    }

    /// The term with this id, if it exists.
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn term(&self, id: i64) -> Result<Option<Term>, DbError> {
        let sql = format!("SELECT {COLUMNS} FROM term WHERE id = ?1");
        Ok(self
            .conn()
            .query_row(&sql, [id], term_from_row)
            .optional()?)
    }

    /// The term key for `text` in a language, or `None` if the language is not stored.
    ///
    /// # Errors
    /// Returns a database error if the language cannot be read.
    /// A phrase is keyed by its words' keys (see `LanguageRules::term_key`).
    pub fn term_key(&self, language_code: &str, text: &str) -> Result<Option<String>, DbError> {
        if let Some(rules) = self.rules(language_code)? {
            return Ok(Some(rules.term_key(text)));
        }
        let Some(language) = self.language(language_code)? else {
            return Ok(None);
        };
        // A language whose word rules do not compile cannot tokenize, so fall
        // back to plain normalisation, as single words always used.
        let substitutions = parse_substitutions(&language.character_substitutions);
        Ok(Some(term_key(text, &substitutions)))
    }

    /// The saved term for this word in this language, if there is one. The word
    /// is matched by its term key, so case and apostrophe style do not matter.
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn find_term(&self, language_code: &str, text: &str) -> Result<Option<Term>, DbError> {
        let Some(key) = self.term_key(language_code, text)? else {
            return Ok(None);
        };
        let sql = format!("SELECT {COLUMNS} FROM term WHERE language_code = ?1 AND text = ?2");
        Ok(self
            .conn()
            .query_row(&sql, params![language_code, key], term_from_row)
            .optional()?)
    }

    /// All saved terms in a language, sorted by text.
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn terms(&self, language_code: &str) -> Result<Vec<Term>, DbError> {
        let sql = format!("SELECT {COLUMNS} FROM term WHERE language_code = ?1 ORDER BY text");
        let mut statement = self.conn().prepare(&sql)?;
        let rows = statement.query_map([language_code], term_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Saves every word in `words` that is not saved yet as well known (99), in
    /// one transaction, and returns the ids of the terms it added, so the
    /// marking can be undone with [`Database::undo_mark_known`]. Words already saved
    /// keep their status, so a word being learned is never marked known by accident.
    ///
    /// `words` are `(term key, form as seen)` pairs, e.g. from a parsed chapter.
    ///
    /// # Errors
    /// Returns `NotFound` if the language is not stored, and a database error if
    /// a statement fails.
    pub fn mark_known(
        &self,
        language_code: &str,
        words: &[(String, String)],
    ) -> Result<Vec<i64>, DbError> {
        if self.language(language_code)?.is_none() {
            return Err(DbError::NotFound {
                what: format!("language {language_code}"),
            });
        }
        let transaction = self.conn().unchecked_transaction()?;
        let mut added = Vec::new();
        {
            // A word that is already saved hits the conflict, so RETURNING gives no row for it.
            let mut insert = transaction.prepare(
                "INSERT INTO term (language_code, text, status, display_text)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT (language_code, text) DO NOTHING
                 RETURNING id",
            )?;
            for (key, display_text) in words {
                if key.trim().is_empty() {
                    continue;
                }
                let mut rows = insert.query(params![
                    language_code,
                    key,
                    Status::WellKnown.code(),
                    display_text
                ])?;
                if let Some(row) = rows.next()? {
                    added.push(row.get(0)?);
                }
            }
        }
        self.record_word_counts(Some(language_code))?;
        transaction.commit()?;
        Ok(added)
    }

    /// Changes only a saved word's status (e.g. a double tap raising its level),
    /// keeping its translation, notes and card. Returns the status it had, or
    /// `None` if the word is not saved.
    ///
    /// # Errors
    /// Returns `NotFound` if the language is not stored, and a database error if
    /// a statement fails.
    pub fn set_term_status(
        &self,
        language_code: &str,
        text: &str,
        status: Status,
    ) -> Result<Option<Status>, DbError> {
        let Some(term) = self.find_term(language_code, text)? else {
            return Ok(None);
        };
        if let Status::Learning(level) = status {
            if !(1..=5).contains(&level) {
                return Err(DbError::InvalidValue(format!("learning level {level}")));
            }
        }
        let transaction = self.conn().unchecked_transaction()?;
        transaction.execute(
            "UPDATE term SET status = ?2 WHERE id = ?1",
            params![term.id, status.code()],
        )?;
        if self.wants_anki_card(term.id, status)? {
            self.queue_for_anki(term.id)?;
        }
        self.record_word_counts(Some(language_code))?;
        transaction.commit()?;
        Ok(Some(term.status))
    }

    /// Undoes [`Database::mark_known`]: deletes the terms it added, except any
    /// that were changed since (another status, a translation or notes, or an
    /// `AnkiDroid` card): those were saved on purpose. Returns how many were deleted.
    ///
    /// # Errors
    /// Returns a database error if a statement fails.
    pub fn undo_mark_known(&self, term_ids: &[i64]) -> Result<usize, DbError> {
        let transaction = self.conn().unchecked_transaction()?;
        let mut deleted = 0;
        {
            let mut delete = transaction.prepare(
                "DELETE FROM term
                 WHERE id = ?1 AND status = ?2 AND translation = '' AND notes = ''
                   AND id NOT IN (SELECT term_id FROM anki_link)",
            )?;
            for id in term_ids {
                deleted += delete.execute(params![id, Status::WellKnown.code()])?;
            }
        }
        self.record_word_counts(None)?;
        transaction.commit()?;
        Ok(deleted)
    }

    /// Deletes a term. Its `AnkiDroid` link and queue entry go with it. A card
    /// Allreads made is queued to be suspended in `AnkiDroid` (see
    /// [`Database::pending_anki_suspends`]); cards of words imported from Anki
    /// are left alone. A copy is kept for a day, for
    /// [`Database::restore_forgotten_term`].
    ///
    /// # Errors
    /// Returns `NotFound` if no term has this id.
    pub fn delete_term(&self, id: i64) -> Result<(), DbError> {
        let term = self.term(id)?.ok_or_else(|| DbError::NotFound {
            what: format!("term {id}"),
        })?;
        let link = self.anki_link(id)?;
        let elsewhere: bool = self.conn().query_row(
            "SELECT card_elsewhere FROM term WHERE id = ?1",
            [id],
            |row| row.get(0),
        )?;
        let transaction = self.conn().unchecked_transaction()?;
        if let (Some(link), false) = (link, elsewhere) {
            transaction.execute(
                "INSERT INTO anki_suspend (note_id, language_code, term_key, queued_at)
                 VALUES (?1, ?2, ?3, CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER))
                 ON CONFLICT (note_id) DO UPDATE SET queued_at = excluded.queued_at",
                params![link.note_id, term.language_code, term.text],
            )?;
        }
        transaction.execute(
            &format!("DELETE FROM forgotten_term WHERE forgotten_at < {NOW_MS} - 86400000"),
            [],
        )?;
        transaction.execute(
            &format!(
                "INSERT OR REPLACE INTO forgotten_term
                     (language_code, text, term_id, display_text, lemma, status, translation, notes,
                      parent_id, context_sentence, context_translation, source_book, source_chapter,
                      imported_from, card_elsewhere, note_id, deck_id, was_queued, forgotten_at)
                 SELECT t.language_code, t.text, t.id, t.display_text, t.lemma, t.status, t.translation,
                        t.notes, t.parent_id, t.context_sentence, t.context_translation, t.source_book,
                        t.source_chapter, t.imported_from, t.card_elsewhere, l.note_id, l.deck_id,
                        EXISTS (SELECT 1 FROM anki_outbox o WHERE o.term_id = t.id), {NOW_MS}
                 FROM term t LEFT JOIN anki_link l ON l.term_id = t.id
                 WHERE t.id = ?1"
            ),
            [id],
        )?;
        transaction.execute("DELETE FROM term WHERE id = ?1", [id])?;
        self.record_word_counts(None)?;
        transaction.commit()?;
        Ok(())
    }

    /// Brings back a word forgotten in the last day exactly as it was: level,
    /// meaning, sentence, import badge, `AnkiDroid` card (whose suspension is
    /// called off) and place in the push queue. Returns its id, or `None` when
    /// there is nothing to bring back, or the word has been saved again since.
    ///
    /// # Errors
    /// Returns a database error if a statement fails.
    pub fn restore_forgotten_term(
        &self,
        language_code: &str,
        text: &str,
    ) -> Result<Option<i64>, DbError> {
        let Some(key) = self.term_key(language_code, text)? else {
            return Ok(None);
        };
        let saved: Option<(Option<i64>, Option<i64>, bool)> = self
            .conn()
            .query_row(
                "SELECT note_id, deck_id, was_queued FROM forgotten_term
                 WHERE language_code = ?1 AND text = ?2",
                params![language_code, key],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let Some((note_id, deck_id, was_queued)) = saved else {
            return Ok(None);
        };
        if self.find_term(language_code, &key)?.is_some() {
            // Saved again in the meantime: that newer word stays as it is.
            self.conn().execute(
                "DELETE FROM forgotten_term WHERE language_code = ?1 AND text = ?2",
                params![language_code, key],
            )?;
            return Ok(None);
        }
        let transaction = self.conn().unchecked_transaction()?;
        // The old id when it is still free (a new one otherwise), and the
        // parent only if it still exists.
        let id: i64 = transaction.query_row(
            "INSERT INTO term
                 (id, language_code, text, display_text, lemma, status, translation, notes, parent_id,
                  context_sentence, context_translation, source_book, source_chapter, imported_from,
                  card_elsewhere)
             SELECT CASE WHEN EXISTS (SELECT 1 FROM term WHERE id = f.term_id) THEN NULL ELSE f.term_id END,
                    f.language_code, f.text, f.display_text, f.lemma, f.status, f.translation, f.notes,
                    CASE WHEN EXISTS (SELECT 1 FROM term WHERE id = f.parent_id) THEN f.parent_id END,
                    f.context_sentence, f.context_translation, f.source_book, f.source_chapter,
                    f.imported_from, f.card_elsewhere
             FROM forgotten_term f WHERE f.language_code = ?1 AND f.text = ?2
             RETURNING id",
            params![language_code, key],
            |row| row.get(0),
        )?;
        transaction.execute(
            "DELETE FROM forgotten_term WHERE language_code = ?1 AND text = ?2",
            params![language_code, key],
        )?;
        if let (Some(note_id), Some(deck_id)) = (note_id, deck_id) {
            self.link_anki_note(id, note_id, deck_id)?;
        }
        if was_queued {
            self.queue_for_anki(id)?;
        }
        self.record_word_counts(None)?;
        transaction.commit()?;
        Ok(Some(id))
    }

    /// Cards of forgotten words to suspend, by note id: those forgotten at least
    /// `min_age_seconds` ago, so a word brought back with Undo is never touched.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn pending_anki_suspends(&self, min_age_seconds: u32) -> Result<Vec<i64>, DbError> {
        let mut statement = self.conn().prepare(
            "SELECT note_id FROM anki_suspend
             WHERE queued_at <= CAST((julianday('now') - 2440587.5) * 86400000 AS INTEGER) - ?1 * 1000
             ORDER BY queued_at",
        )?;
        let rows = statement.query_map([min_age_seconds], |row| row.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The card of `note_id` was suspended (or no longer exists): done.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn record_anki_suspended(&self, note_id: i64) -> Result<(), DbError> {
        self.conn()
            .execute("DELETE FROM anki_suspend WHERE note_id = ?1", [note_id])?;
        Ok(())
    }
}

fn term_from_row(row: &Row) -> rusqlite::Result<Term> {
    let code: i64 = row.get(4)?;
    let status = u8::try_from(code)
        .ok()
        .and_then(Status::from_code)
        .ok_or(rusqlite::Error::IntegralValueOutOfRange(4, code))?;
    Ok(Term {
        id: row.get(0)?,
        language_code: row.get(1)?,
        text: row.get(2)?,
        lemma: row.get(3)?,
        status,
        translation: row.get(5)?,
        notes: row.get(6)?,
        parent_id: row.get(7)?,
        display_text: row.get(8)?,
        context: TermContext {
            sentence: row.get(9)?,
            sentence_translation: row.get(10)?,
            source_book: row.get(11)?,
            source_chapter: row.get(12)?,
        },
        imported_from: row.get(13)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests_support::{french_db, new_term};

    fn context(sentence: &str) -> TermContext {
        TermContext {
            sentence: sentence.into(),
            sentence_translation: "The man reads.".into(),
            source_book: "Le Livre".into(),
            source_chapter: "Chapitre un".into(),
        }
    }

    #[test]
    fn context_is_stored_and_kept_when_re_saving_without_one() {
        let db = french_db();
        let mut term = new_term("homme", Status::Learning(1));
        term.context = Some(context("L'homme lit."));
        let id = db.save_term(&term).unwrap();
        db.save_term(&new_term("homme", Status::Learning(2)))
            .unwrap();
        let saved = db.term(id).unwrap().unwrap();
        assert_eq!(saved.context, context("L'homme lit."));
        assert_eq!(saved.status, Status::Learning(2));
        // A new context replaces the old one.
        let mut again = new_term("homme", Status::Learning(2));
        again.context = Some(context("Un homme dort."));
        db.save_term(&again).unwrap();
        assert_eq!(
            db.term(id).unwrap().unwrap().context.sentence,
            "Un homme dort."
        );
    }

    #[test]
    fn a_phrase_is_keyed_by_its_words_so_punctuation_and_spaces_do_not_matter() {
        let db = french_db();
        db.upsert_language(
            &crate::presets::builtin_presets()
                .unwrap()
                .into_iter()
                .find(|p| p.code == "fr")
                .unwrap(),
        )
        .unwrap();
        let id = db
            .save_term(&new_term("Pierre  précieuse", Status::Learning(1)))
            .unwrap();
        assert_eq!(db.term(id).unwrap().unwrap().text, "pierre précieuse");
        assert_eq!(
            db.term_key("fr", "d’une pierre, précieuse")
                .unwrap()
                .as_deref(),
            Some("d'une pierre précieuse")
        );
        // A single word keeps its key; trailing punctuation is not part of it.
        assert_eq!(db.term_key("fr", "Chat.").unwrap().as_deref(), Some("chat"));
    }

    #[test]
    fn changing_a_language_changes_its_keys_at_once() {
        let db = french_db();
        let mut french = crate::presets::builtin_presets()
            .unwrap()
            .into_iter()
            .find(|p| p.code == "fr")
            .unwrap();
        db.upsert_language(&french).unwrap();
        assert_eq!(
            db.term_key("fr", "précieuse").unwrap().as_deref(),
            Some("précieuse")
        );
        // Without accented letters as word characters, "é" splits the word;
        // the rules cached for the first preset must not linger.
        french.word_characters = "a-zA-Z".into();
        db.upsert_language(&french).unwrap();
        assert_eq!(
            db.term_key("fr", "précieuse").unwrap().as_deref(),
            Some("pr cieuse")
        );
        assert_eq!(db.term_key("xx", "a").unwrap(), None);
    }

    #[test]
    fn only_levels_one_to_four_are_queued_for_anki() {
        let db = french_db();
        let mut queued = Vec::new();
        for (text, status) in [
            ("un", Status::Learning(1)),
            ("deux", Status::Learning(4)),
            ("cinq", Status::Learning(5)),
            ("connu", Status::WellKnown),
            ("ignore", Status::Ignored),
        ] {
            let id = db.save_term(&new_term(text, status)).unwrap();
            if status.makes_anki_card() {
                queued.push(id);
            }
        }
        let ids: Vec<i64> = db
            .anki_queue()
            .unwrap()
            .into_iter()
            .map(|q| q.term_id)
            .collect();
        assert_eq!(ids, queued);
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn a_word_with_a_card_is_queued_again_when_edited_even_at_level_five() {
        let db = french_db();
        let id = db
            .save_term(&new_term("chat", Status::Learning(2)))
            .unwrap();
        db.link_anki_note(id, 1001, 1).unwrap();
        db.complete_anki_push(id).unwrap();
        db.save_term(&new_term("chat", Status::Learning(5)))
            .unwrap();
        assert_eq!(db.anki_queue().unwrap()[0].term_id, id);
    }

    #[test]
    fn status_codes_round_trip() {
        for code in 0..=255u8 {
            match Status::from_code(code) {
                Some(status) => assert_eq!(status.code(), code),
                None => assert!(!matches!(code, 1..=5 | 98 | 99)),
            }
        }
    }

    #[test]
    fn learning_levels_outside_one_to_five_are_refused() {
        assert_eq!(Status::learning(0), None);
        assert_eq!(Status::learning(6), None);
        assert_eq!(Status::learning(3), Some(Status::Learning(3)));
    }

    #[test]
    fn saved_term_can_be_read_back() {
        let db = french_db();
        let mut term = new_term("chat", Status::Learning(2));
        term.translation = "cat".into();
        let id = db.save_term(&term).unwrap();
        let saved = db.term(id).unwrap().unwrap();
        assert_eq!(saved.text, "chat");
        assert_eq!(saved.status, Status::Learning(2));
        assert_eq!(saved.translation, "cat");
    }

    #[test]
    fn saving_the_same_word_updates_the_row_instead_of_duplicating() {
        let db = french_db();
        let first = db
            .save_term(&new_term("chat", Status::Learning(1)))
            .unwrap();
        let mut update = new_term("chat", Status::Learning(4));
        update.translation = "cat".into();
        let second = db.save_term(&update).unwrap();
        assert_eq!(first, second);
        assert_eq!(db.terms("fr").unwrap().len(), 1);
        assert_eq!(db.term(first).unwrap().unwrap().status, Status::Learning(4));
    }

    #[test]
    fn find_term_matches_language_and_text() {
        let db = french_db();
        let id = db.save_term(&new_term("chat", Status::Ignored)).unwrap();
        assert_eq!(db.find_term("fr", "chat").unwrap().map(|t| t.id), Some(id));
        assert_eq!(db.find_term("de", "chat").unwrap(), None);
        assert_eq!(db.find_term("fr", "chien").unwrap(), None);
    }

    #[test]
    fn terms_are_sorted_by_text() {
        let db = french_db();
        db.save_term(&new_term("chien", Status::Learning(1)))
            .unwrap();
        db.save_term(&new_term("abeille", Status::Learning(1)))
            .unwrap();
        let texts: Vec<String> = db
            .terms("fr")
            .unwrap()
            .into_iter()
            .map(|t| t.text)
            .collect();
        assert_eq!(texts, ["abeille", "chien"]);
    }

    #[test]
    fn blank_text_is_refused() {
        let db = french_db();
        assert!(matches!(
            db.save_term(&new_term("  ", Status::Learning(1))),
            Err(DbError::InvalidValue(_))
        ));
    }

    #[test]
    fn database_refuses_a_status_outside_the_allowed_codes() {
        let db = french_db();
        let result = db.conn().execute(
            "INSERT INTO term (language_code, text, status) VALUES ('fr', 'x', 7)",
            [],
        );
        assert!(result.is_err());
    }

    #[test]
    fn a_term_cannot_be_its_own_lemma() {
        let db = french_db();
        let id = db
            .save_term(&new_term("chat", Status::Learning(1)))
            .unwrap();
        let mut selfish = new_term("chat", Status::Learning(1));
        selfish.parent_id = Some(id);
        assert!(matches!(
            db.save_term(&selfish),
            Err(DbError::InvalidValue(_))
        ));
    }

    #[test]
    fn lemma_links_cannot_form_a_loop() {
        let db = french_db();
        let mangeons = db
            .save_term(&new_term("mangeons", Status::Learning(1)))
            .unwrap();
        let mut mange = new_term("mange", Status::Learning(1));
        mange.parent_id = Some(mangeons);
        db.save_term(&mange).unwrap();
        let mut back = new_term("mangeons", Status::Learning(1));
        back.parent_id = db.find_term("fr", "mange").unwrap().map(|t| t.id);
        assert!(matches!(db.save_term(&back), Err(DbError::InvalidValue(_))));
    }

    #[test]
    fn a_normal_lemma_chain_is_accepted() {
        let db = french_db();
        let lemma = db
            .save_term(&new_term("manger", Status::Learning(1)))
            .unwrap();
        let mut form = new_term("mange", Status::Learning(1));
        form.parent_id = Some(lemma);
        assert!(db.save_term(&form).is_ok());
    }

    #[test]
    fn surrounding_spaces_do_not_make_a_second_term() {
        let db = french_db();
        let first = db
            .save_term(&new_term("chat", Status::Learning(1)))
            .unwrap();
        let second = db
            .save_term(&new_term(" chat ", Status::Learning(2)))
            .unwrap();
        assert_eq!(first, second);
        assert_eq!(db.terms("fr").unwrap().len(), 1);
        assert_eq!(
            db.find_term("fr", " chat").unwrap().map(|t| t.id),
            Some(first)
        );
    }

    #[test]
    fn an_invalid_learning_level_is_a_typed_error() {
        let db = french_db();
        assert!(matches!(
            db.save_term(&new_term("chat", Status::Learning(9))),
            Err(DbError::InvalidValue(_))
        ));
    }

    #[test]
    fn case_does_not_make_a_second_term_and_the_first_form_is_kept() {
        let db = french_db();
        let first = db
            .save_term(&new_term("Chat", Status::Learning(1)))
            .unwrap();
        let second = db
            .save_term(&new_term("chat", Status::Learning(3)))
            .unwrap();
        assert_eq!(first, second);
        let term = db.term(first).unwrap().unwrap();
        assert_eq!(
            (term.text.as_str(), term.display_text.as_str()),
            ("chat", "Chat")
        );
        assert_eq!(term.status, Status::Learning(3));
        assert_eq!(
            db.find_term("fr", "CHAT").unwrap().map(|t| t.id),
            Some(first)
        );
    }

    #[test]
    fn curly_and_straight_apostrophes_find_the_same_term() {
        let db = french_db();
        let id = db
            .save_term(&new_term("aujourd’hui", Status::Learning(1)))
            .unwrap();
        assert_eq!(
            db.find_term("fr", "aujourd'hui").unwrap().map(|t| t.id),
            Some(id)
        );
    }

    #[test]
    fn saving_in_an_unknown_language_is_a_typed_not_found() {
        let db = french_db();
        let mut term = new_term("Haus", Status::Learning(1));
        term.language_code = "zz".into();
        assert!(matches!(db.save_term(&term), Err(DbError::NotFound { .. })));
    }

    fn pairs(words: &[&str]) -> Vec<(String, String)> {
        words
            .iter()
            .map(|w| (w.to_lowercase(), (*w).to_string()))
            .collect()
    }

    #[test]
    fn mark_known_adds_only_unsaved_words() {
        let db = french_db();
        let learning = db
            .save_term(&new_term("chat", Status::Learning(2)))
            .unwrap();
        let added = db
            .mark_known("fr", &pairs(&["chat", "Chien", "chien", "oiseau"]))
            .unwrap();
        assert_eq!(added.len(), 2);
        assert_eq!(
            db.term(learning).unwrap().unwrap().status,
            Status::Learning(2)
        );
        let chien = db.find_term("fr", "chien").unwrap().unwrap();
        assert_eq!(
            (chien.status, chien.display_text.as_str()),
            (Status::WellKnown, "Chien")
        );
    }

    #[test]
    fn undoing_mark_known_keeps_words_changed_since_and_older_ones() {
        let db = french_db();
        db.save_term(&new_term("chat", Status::WellKnown)).unwrap();
        let added = db
            .mark_known("fr", &pairs(&["chat", "chien", "oiseau"]))
            .unwrap();
        // "oiseau" was saved at a level after the marking: it stays.
        db.save_term(&new_term("oiseau", Status::Learning(2)))
            .unwrap();
        assert_eq!(db.undo_mark_known(&added).unwrap(), 1);
        assert!(db.find_term("fr", "chien").unwrap().is_none());
        assert!(db.find_term("fr", "chat").unwrap().is_some());
        assert_eq!(
            db.find_term("fr", "oiseau").unwrap().unwrap().status,
            Status::Learning(2)
        );
    }

    #[test]
    fn undoing_mark_known_keeps_a_known_word_given_a_translation_since() {
        let db = french_db();
        let added = db.mark_known("fr", &pairs(&["chien"])).unwrap();
        let mut edited = new_term("chien", Status::WellKnown);
        edited.translation = "dog".into();
        db.save_term(&edited).unwrap();
        assert_eq!(db.undo_mark_known(&added).unwrap(), 0);
        assert!(db.find_term("fr", "chien").unwrap().is_some());
    }

    #[test]
    fn setting_a_status_keeps_the_rest() {
        let db = french_db();
        let mut chat = new_term("chat", Status::Learning(2));
        chat.translation = "cat".into();
        db.save_term(&chat).unwrap();
        let before = db
            .set_term_status("fr", "Chat", Status::Learning(3))
            .unwrap();
        assert_eq!(before, Some(Status::Learning(2)));
        let saved = db.find_term("fr", "chat").unwrap().unwrap();
        assert_eq!(
            (saved.status, saved.translation.as_str()),
            (Status::Learning(3), "cat")
        );
        assert_eq!(
            db.set_term_status("fr", "chien", Status::WellKnown)
                .unwrap(),
            None
        );
    }

    #[test]
    fn forgetting_a_word_with_a_card_queues_its_suspension_unless_saved_again() {
        let db = french_db();
        let chat = db
            .save_term(&new_term("chat", Status::Learning(1)))
            .unwrap();
        let chien = db
            .save_term(&new_term("chien", Status::Learning(1)))
            .unwrap();
        db.save_term(&new_term("oiseau", Status::Learning(1)))
            .unwrap();
        db.link_anki_note(chat, 101, 1).unwrap();
        db.link_anki_note(chien, 102, 1).unwrap();
        db.delete_term(chat).unwrap();
        db.delete_term(chien).unwrap();
        // A word without a card queues nothing.
        let oiseau = db.find_term("fr", "oiseau").unwrap().unwrap().id;
        db.delete_term(oiseau).unwrap();
        // Undo brings "chien" back before the suspension ran.
        db.save_term(&new_term("chien", Status::Learning(1)))
            .unwrap();
        assert_eq!(db.pending_anki_suspends(0).unwrap(), [101]);
        // Not before the Undo window has passed.
        assert!(db.pending_anki_suspends(60).unwrap().is_empty());
        db.record_anki_suspended(101).unwrap();
        assert!(db.pending_anki_suspends(0).unwrap().is_empty());
    }

    #[test]
    fn mark_known_in_an_unknown_language_is_not_found() {
        let db = french_db();
        assert!(matches!(
            db.mark_known("zz", &pairs(&["x"])),
            Err(DbError::NotFound { .. })
        ));
    }

    #[test]
    fn re_saving_without_a_parent_keeps_the_existing_link() {
        let db = french_db();
        let lemma = db
            .save_term(&new_term("manger", Status::Learning(1)))
            .unwrap();
        let mut form = new_term("mange", Status::Learning(1));
        form.parent_id = Some(lemma);
        let id = db.save_term(&form).unwrap();
        db.save_term(&new_term("mange", Status::Learning(3)))
            .unwrap();
        let saved = db.term(id).unwrap().unwrap();
        assert_eq!(
            (saved.parent_id, saved.status),
            (Some(lemma), Status::Learning(3))
        );
    }

    #[test]
    fn deleting_a_parent_clears_the_link_on_its_children() {
        let db = french_db();
        let lemma = db
            .save_term(&new_term("manger", Status::Learning(1)))
            .unwrap();
        let mut form = new_term("mange", Status::Learning(1));
        form.parent_id = Some(lemma);
        let form_id = db.save_term(&form).unwrap();
        db.delete_term(lemma).unwrap();
        assert_eq!(db.term(form_id).unwrap().unwrap().parent_id, None);
    }

    #[test]
    fn deleting_a_missing_term_is_not_found() {
        let db = french_db();
        assert!(matches!(db.delete_term(5), Err(DbError::NotFound { .. })));
    }

    #[test]
    fn a_forgotten_word_comes_back_whole() {
        let db = french_db();
        let mut word = new_term("Perle", Status::Learning(3));
        word.translation = "pearl".into();
        word.context = Some(TermContext {
            sentence: "Une perle rare.".into(),
            sentence_translation: "A rare pearl.".into(),
            source_book: "Un".into(),
            source_chapter: "I".into(),
        });
        let id = db.save_term(&word).unwrap();
        db.link_anki_note(id, 1001, 77).unwrap();
        db.queue_for_anki(id).unwrap();
        let before = db.term(id).unwrap().unwrap();

        db.delete_term(id).unwrap();
        assert_eq!(db.pending_anki_suspends(0).unwrap(), [1001]);
        assert_eq!(db.restore_forgotten_term("fr", "perle").unwrap(), Some(id));

        assert_eq!(db.term(id).unwrap().unwrap(), before);
        assert_eq!(db.anki_link(id).unwrap().map(|l| l.note_id), Some(1001));
        assert!(
            db.pending_anki_suspends(0).unwrap().is_empty(),
            "the card is not suspended"
        );
        assert_eq!(db.anki_queue().unwrap().len(), 1);
        // Only once.
        assert_eq!(db.restore_forgotten_term("fr", "perle").unwrap(), None);
    }

    #[test]
    fn an_imported_word_keeps_its_badge_and_stays_without_a_card() {
        let db = french_db();
        let imported = crate::db::import::ImportedWord {
            text: "chat".into(),
            meaning: Some("cat".into()),
            status: Status::WellKnown,
        };
        db.import_words("fr", "Anki · Vocabulaire", true, &[imported])
            .unwrap();
        let id = db.find_term("fr", "chat").unwrap().unwrap().id;

        db.delete_term(id).unwrap();
        assert!(db.pending_anki_suspends(0).unwrap().is_empty());
        db.restore_forgotten_term("fr", "chat").unwrap().unwrap();

        let back = db.find_term("fr", "chat").unwrap().unwrap();
        assert_eq!(back.imported_from, "Anki · Vocabulaire");
        let elsewhere: bool = db
            .conn()
            .query_row(
                "SELECT card_elsewhere FROM term WHERE id = ?1",
                [back.id],
                |row| row.get(0),
            )
            .unwrap();
        assert!(elsewhere);
    }

    #[test]
    fn a_word_saved_again_is_not_replaced_by_its_forgotten_copy() {
        let db = french_db();
        let id = db
            .save_term(&new_term("chien", Status::Learning(1)))
            .unwrap();
        db.delete_term(id).unwrap();
        db.save_term(&new_term("chien", Status::Learning(4)))
            .unwrap();

        assert_eq!(db.restore_forgotten_term("fr", "chien").unwrap(), None);
        assert_eq!(
            db.find_term("fr", "chien").unwrap().unwrap().status,
            Status::Learning(4)
        );
    }

    #[test]
    fn nothing_to_restore_for_a_word_never_forgotten() {
        let db = french_db();
        assert_eq!(db.restore_forgotten_term("fr", "oiseau").unwrap(), None);
    }
}
