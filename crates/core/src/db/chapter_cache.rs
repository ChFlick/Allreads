//! Cached parsed chapters, and the status lookup used when a chapter opens.

use std::collections::HashMap;

use rusqlite::{params, OptionalExtension};

use super::term::Status;
use super::{Database, DbError};

impl Database {
    /// The cached data for a chapter, if it was stored by this parser version.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn cached_chapter(
        &self,
        book_id: i64,
        chapter_index: u32,
        parser_version: i64,
    ) -> Result<Option<Vec<u8>>, DbError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT data FROM chapter_cache
                 WHERE book_id = ?1 AND chapter_index = ?2 AND parser_version = ?3",
                params![book_id, chapter_index, parser_version],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Stores a parsed chapter, replacing any older copy.
    ///
    /// # Errors
    /// Returns a database error if the book does not exist or the statement fails.
    pub fn store_chapter_cache(
        &self,
        book_id: i64,
        chapter_index: u32,
        parser_version: i64,
        data: &[u8],
    ) -> Result<(), DbError> {
        self.conn().execute(
            "INSERT INTO chapter_cache (book_id, chapter_index, parser_version, data)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (book_id, chapter_index) DO UPDATE SET
                 parser_version = excluded.parser_version,
                 data = excluded.data",
            params![book_id, chapter_index, parser_version, data],
        )?;
        Ok(())
    }

    /// Every saved term's status in a language, by term key. One query, so a
    /// chapter's statuses are joined without a lookup per word.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn term_statuses(&self, language_code: &str) -> Result<HashMap<String, Status>, DbError> {
        let mut statement = self
            .conn()
            .prepare("SELECT text, status FROM term WHERE language_code = ?1")?;
        let rows = statement.query_map([language_code], |row| {
            let key: String = row.get(0)?;
            let code: i64 = row.get(1)?;
            Ok((key, code))
        })?;
        let mut statuses = HashMap::new();
        for row in rows {
            let (key, code) = row?;
            let status = u8::try_from(code)
                .ok()
                .and_then(Status::from_code)
                .ok_or_else(|| DbError::InvalidValue(format!("status {code} for {key}")))?;
            statuses.insert(key, status);
        }
        Ok(statuses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::book::NewBook;
    use crate::db::term::NewTerm;
    use crate::db::tests_support::french_db;

    fn db_with_book() -> (Database, i64) {
        let db = french_db();
        let id = db
            .insert_book(&NewBook {
                title: "Un".into(),
                author: None,
                language_code: "fr".into(),
                file_path: "un.epub".into(),
                cover_path: None,
            })
            .unwrap();
        (db, id)
    }

    #[test]
    fn cache_is_returned_only_for_the_same_parser_version() {
        let (db, book) = db_with_book();
        db.store_chapter_cache(book, 0, 1, b"v1").unwrap();
        assert_eq!(db.cached_chapter(book, 0, 1).unwrap(), Some(b"v1".to_vec()));
        assert_eq!(db.cached_chapter(book, 0, 2).unwrap(), None);
        assert_eq!(db.cached_chapter(book, 1, 1).unwrap(), None);
    }

    #[test]
    fn storing_again_replaces_the_old_copy() {
        let (db, book) = db_with_book();
        db.store_chapter_cache(book, 0, 1, b"old").unwrap();
        db.store_chapter_cache(book, 0, 2, b"new").unwrap();
        assert_eq!(
            db.cached_chapter(book, 0, 2).unwrap(),
            Some(b"new".to_vec())
        );
        assert_eq!(db.cached_chapter(book, 0, 1).unwrap(), None);
    }

    #[test]
    fn cache_goes_with_its_book() {
        let (db, book) = db_with_book();
        db.store_chapter_cache(book, 0, 1, b"x").unwrap();
        db.delete_book(book).unwrap();
        assert_eq!(db.cached_chapter(book, 0, 1).unwrap(), None);
    }

    #[test]
    fn statuses_are_keyed_by_term_key() {
        let db = french_db();
        for (text, status) in [("Chat", Status::Learning(2)), ("chien", Status::WellKnown)] {
            db.save_term(&NewTerm {
                language_code: "fr".into(),
                text: text.into(),
                lemma: None,
                status,
                translation: String::new(),
                notes: String::new(),
                parent_id: None,
            })
            .unwrap();
        }
        let statuses = db.term_statuses("fr").unwrap();
        assert_eq!(statuses.get("chat"), Some(&Status::Learning(2)));
        assert_eq!(statuses.get("chien"), Some(&Status::WellKnown));
        assert!(db.term_statuses("de").unwrap().is_empty());
    }
}
