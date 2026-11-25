//! `AnkiDroid` links and the queue of terms waiting to be pushed.

use rusqlite::{params, OptionalExtension, Row};

use super::{require_one_changed, u32_column, Database, DbError};

/// The `AnkiDroid` note a term was pushed as.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnkiLink {
    pub term_id: i64,
    pub note_id: i64,
    pub deck_id: i64,
}

/// A term waiting to be pushed to `AnkiDroid`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueuedTerm {
    pub term_id: i64,
    pub attempts: u32,
}

impl Database {
    /// Records which `AnkiDroid` note a term was pushed as. Calling again for the
    /// same term replaces the link, so edits update the same card.
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn link_anki_note(&self, term_id: i64, note_id: i64, deck_id: i64) -> Result<(), DbError> {
        self.conn().execute(
            "INSERT INTO anki_link (term_id, note_id, deck_id) VALUES (?1, ?2, ?3)
             ON CONFLICT (term_id) DO UPDATE SET
                 note_id = excluded.note_id,
                 deck_id = excluded.deck_id,
                 last_pushed_at = datetime('now')",
            params![term_id, note_id, deck_id],
        )?;
        Ok(())
    }

    /// The `AnkiDroid` note linked to a term, if it has been pushed.
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn anki_link(&self, term_id: i64) -> Result<Option<AnkiLink>, DbError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT term_id, note_id, deck_id FROM anki_link WHERE term_id = ?1",
                [term_id],
                link_from_row,
            )
            .optional()?)
    }

    /// Queues a term for pushing. Queuing a term that is already waiting does nothing.
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn queue_for_anki(&self, term_id: i64) -> Result<(), DbError> {
        self.conn().execute(
            "INSERT OR IGNORE INTO anki_outbox (term_id) VALUES (?1)",
            [term_id],
        )?;
        Ok(())
    }

    /// Terms waiting to be pushed, oldest first.
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn anki_queue(&self) -> Result<Vec<QueuedTerm>, DbError> {
        let mut statement = self
            .conn()
            .prepare("SELECT term_id, attempts FROM anki_outbox ORDER BY id")?;
        let rows = statement.query_map([], queued_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Counts a failed push attempt for a queued term.
    /// # Errors
    /// Returns `NotFound` if no row matches, and a database error if the statement fails.
    pub fn record_anki_attempt(&self, term_id: i64) -> Result<(), DbError> {
        let changed = self.conn().execute(
            "UPDATE anki_outbox SET attempts = attempts + 1 WHERE term_id = ?1",
            [term_id],
        )?;
        require_one_changed(changed, || format!("queued term {term_id}"))
    }

    /// Removes a term from the queue after it has been pushed.
    /// # Errors
    /// Returns `NotFound` if no row matches, and a database error if the statement fails.
    pub fn complete_anki_push(&self, term_id: i64) -> Result<(), DbError> {
        let changed = self
            .conn()
            .execute("DELETE FROM anki_outbox WHERE term_id = ?1", [term_id])?;
        require_one_changed(changed, || format!("queued term {term_id}"))
    }
}

fn link_from_row(row: &Row) -> rusqlite::Result<AnkiLink> {
    Ok(AnkiLink {
        term_id: row.get(0)?,
        note_id: row.get(1)?,
        deck_id: row.get(2)?,
    })
}

fn queued_from_row(row: &Row) -> rusqlite::Result<QueuedTerm> {
    Ok(QueuedTerm {
        term_id: row.get(0)?,
        attempts: u32_column(row, 1)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::term::{NewTerm, Status};
    use crate::db::tests_support::french_db;

    fn db_with_term() -> (Database, i64) {
        let db = french_db();
        let term_id = db
            .save_term(&NewTerm {
                language_code: "fr".into(),
                text: "chat".into(),
                lemma: None,
                status: Status::Learning(1),
                translation: "cat".into(),
                notes: String::new(),
                parent_id: None,
            })
            .unwrap();
        (db, term_id)
    }

    #[test]
    fn linking_a_term_records_its_note() {
        let (db, term) = db_with_term();
        db.link_anki_note(term, 1001, 77).unwrap();
        assert_eq!(
            db.anki_link(term).unwrap(),
            Some(AnkiLink {
                term_id: term,
                note_id: 1001,
                deck_id: 77
            })
        );
    }

    #[test]
    fn relinking_replaces_the_note_instead_of_adding_one() {
        let (db, term) = db_with_term();
        db.link_anki_note(term, 1001, 77).unwrap();
        db.link_anki_note(term, 2002, 77).unwrap();
        assert_eq!(db.anki_link(term).unwrap().unwrap().note_id, 2002);
    }

    #[test]
    fn queuing_twice_keeps_one_entry() {
        let (db, term) = db_with_term();
        db.queue_for_anki(term).unwrap();
        db.queue_for_anki(term).unwrap();
        assert_eq!(db.anki_queue().unwrap().len(), 1);
    }

    #[test]
    fn failed_attempts_are_counted_then_the_term_is_completed() {
        let (db, term) = db_with_term();
        db.queue_for_anki(term).unwrap();
        db.record_anki_attempt(term).unwrap();
        db.record_anki_attempt(term).unwrap();
        assert_eq!(db.anki_queue().unwrap()[0].attempts, 2);
        db.complete_anki_push(term).unwrap();
        assert!(db.anki_queue().unwrap().is_empty());
    }

    #[test]
    fn queue_keeps_the_order_terms_were_added() {
        let db = french_db();
        let mut ids = Vec::new();
        for text in ["un", "deux", "trois"] {
            let id = db
                .save_term(&NewTerm {
                    language_code: "fr".into(),
                    text: text.into(),
                    lemma: None,
                    status: Status::Learning(1),
                    translation: String::new(),
                    notes: String::new(),
                    parent_id: None,
                })
                .unwrap();
            db.queue_for_anki(id).unwrap();
            ids.push(id);
        }
        let queued: Vec<i64> = db
            .anki_queue()
            .unwrap()
            .into_iter()
            .map(|q| q.term_id)
            .collect();
        assert_eq!(queued, ids);
    }

    #[test]
    fn deleting_a_term_removes_its_link_and_queue_entry() {
        let (db, term) = db_with_term();
        db.link_anki_note(term, 1001, 77).unwrap();
        db.queue_for_anki(term).unwrap();
        db.delete_term(term).unwrap();
        assert_eq!(db.anki_link(term).unwrap(), None);
        assert!(db.anki_queue().unwrap().is_empty());
    }
}
