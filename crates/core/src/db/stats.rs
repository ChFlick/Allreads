//! Reading statistics per day and book.

use rusqlite::{params, OptionalExtension, Row};

use super::{u32_column, Database, DbError};

/// What was read in one book on one day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadingStat {
    /// The day as `YYYY-MM-DD`.
    pub day: String,
    pub book_id: i64,
    pub words_read: u32,
    pub new_terms: u32,
}

impl Database {
    /// Adds to the day's totals for a book. Totals accumulate across calls.
    ///
    /// # Errors
    /// Returns `InvalidValue` if `day` is not `YYYY-MM-DD`.
    pub fn add_reading(
        &self,
        day: &str,
        book_id: i64,
        words_read: u32,
        new_terms: u32,
    ) -> Result<(), DbError> {
        check_day(day)?;
        self.conn().execute(
            "INSERT INTO reading_stat (day, book_id, words_read, new_terms)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT (day, book_id) DO UPDATE SET
                 words_read = words_read + excluded.words_read,
                 new_terms = new_terms + excluded.new_terms",
            params![day, book_id, words_read, new_terms],
        )?;
        Ok(())
    }

    /// The totals for one book on one day, if anything was read.
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn reading_stat(&self, day: &str, book_id: i64) -> Result<Option<ReadingStat>, DbError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT day, book_id, words_read, new_terms FROM reading_stat
                 WHERE day = ?1 AND book_id = ?2",
                params![day, book_id],
                stat_from_row,
            )
            .optional()?)
    }
}

/// Accepts only `YYYY-MM-DD`, so days sort and compare as text.
fn check_day(day: &str) -> Result<(), DbError> {
    let bytes = day.as_bytes();
    let shape_ok = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit());
    let month: u32 = day.get(5..7).and_then(|m| m.parse().ok()).unwrap_or(0);
    let date: u32 = day.get(8..10).and_then(|d| d.parse().ok()).unwrap_or(0);
    if shape_ok && (1..=12).contains(&month) && (1..=31).contains(&date) {
        Ok(())
    } else {
        Err(DbError::InvalidValue(format!(
            "day '{day}' is not a date YYYY-MM-DD"
        )))
    }
}

fn stat_from_row(row: &Row) -> rusqlite::Result<ReadingStat> {
    Ok(ReadingStat {
        day: row.get(0)?,
        book_id: row.get(1)?,
        words_read: u32_column(row, 2)?,
        new_terms: u32_column(row, 3)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::book::NewBook;
    use crate::db::tests_support::french_db;

    fn db_with_book() -> (Database, i64) {
        let db = french_db();
        let book_id = db
            .insert_book(&NewBook {
                title: "Un".into(),
                author: None,
                language_code: "fr".into(),
                file_path: "un.epub".into(),
                cover_path: None,
            })
            .unwrap();
        (db, book_id)
    }

    #[test]
    fn totals_accumulate_within_a_day() {
        let (db, book) = db_with_book();
        db.add_reading("2026-11-24", book, 120, 3).unwrap();
        db.add_reading("2026-11-24", book, 80, 1).unwrap();
        let stat = db.reading_stat("2026-11-24", book).unwrap().unwrap();
        assert_eq!((stat.words_read, stat.new_terms), (200, 4));
    }

    #[test]
    fn days_are_kept_apart() {
        let (db, book) = db_with_book();
        db.add_reading("2026-11-23", book, 10, 0).unwrap();
        db.add_reading("2026-11-24", book, 20, 0).unwrap();
        assert_eq!(
            db.reading_stat("2026-11-23", book)
                .unwrap()
                .unwrap()
                .words_read,
            10
        );
    }

    #[test]
    fn day_without_reading_is_none() {
        let (db, book) = db_with_book();
        assert_eq!(db.reading_stat("2026-01-01", book).unwrap(), None);
    }

    #[test]
    fn malformed_days_are_refused() {
        let (db, book) = db_with_book();
        for day in ["24-11-2026", "2026/11/24", "2026-11-2x", "today", ""] {
            assert!(
                matches!(
                    db.add_reading(day, book, 1, 0),
                    Err(DbError::InvalidValue(_))
                ),
                "{day:?} should be refused"
            );
        }
    }

    #[test]
    fn impossible_calendar_dates_are_refused() {
        let (db, book) = db_with_book();
        for day in ["2026-13-01", "2026-00-10", "2026-11-00", "2026-11-32"] {
            assert!(
                matches!(
                    db.add_reading(day, book, 1, 0),
                    Err(DbError::InvalidValue(_))
                ),
                "{day} should be refused"
            );
        }
    }

    #[test]
    fn stats_are_removed_with_their_book() {
        let (db, book) = db_with_book();
        db.add_reading("2026-11-24", book, 5, 0).unwrap();
        db.delete_book(book).unwrap();
        assert_eq!(db.reading_stat("2026-11-24", book).unwrap(), None);
    }
}
