//! Progress: how hard a book is (its share of new words), and how many words
//! are known per day.

use std::collections::{HashMap, HashSet};

use rusqlite::params;

use super::{Database, DbError};
use crate::word_tally::BookWord;

/// A book's words compared with the saved ones, names left out. Counts are
/// running words ("le" twice is 2) except the `unique_` ones, which count a
/// word and its forms once ("lire", "lisait"). A form whose dictionary form is
/// saved counts as that word.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Difficulty {
    pub words: u64,
    /// Words not saved at all.
    pub new_words: u64,
    /// Words at learning levels 1–5.
    pub learning_words: u64,
    pub unique_words: u64,
    pub unique_new_words: u64,
}

impl Difficulty {
    /// The share of running words that are new, from 0 to 100.
    #[must_use]
    pub fn new_percent(&self) -> f64 {
        if self.words == 0 {
            0.0
        } else {
            // Word counts stay far below 2^52, so the conversion is exact.
            #[allow(clippy::cast_precision_loss)]
            let share = self.new_words as f64 / self.words as f64;
            share * 100.0
        }
    }
}

/// Saved words in a language at the end of one day.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DayCount {
    /// `YYYY-MM-DD`.
    pub day: String,
    pub known: u32,
    pub learning: u32,
}

impl Database {
    /// The parser version a book's words were counted with, or `None` if
    /// they were not counted yet.
    ///
    /// # Errors
    /// Returns `NotFound` if the book does not exist.
    pub fn book_words_counted_with(&self, book_id: i64) -> Result<Option<i64>, DbError> {
        use rusqlite::OptionalExtension;
        let version: Option<i64> = self
            .conn()
            .query_row(
                "SELECT words_counted_with FROM book WHERE id = ?1",
                [book_id],
                |row| row.get(0),
            )
            .optional()?;
        match version {
            None => Err(DbError::NotFound {
                what: format!("book {book_id}"),
            }),
            Some(0) => Ok(None),
            Some(version) => Ok(Some(version)),
        }
    }

    /// Replaces a book's counted words (term key → count, form, name).
    /// `version` says which parser and counting rules made them.
    ///
    /// # Errors
    /// Returns a database error if a statement fails.
    pub fn store_book_words(
        &self,
        book_id: i64,
        version: i64,
        words: &HashMap<String, BookWord>,
    ) -> Result<(), DbError> {
        let transaction = self.conn().unchecked_transaction()?;
        transaction.execute("DELETE FROM book_word WHERE book_id = ?1", [book_id])?;
        {
            let mut insert = transaction.prepare(
                "INSERT INTO book_word (book_id, key, count, lemma_key, is_name)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
            )?;
            for (key, word) in words {
                insert.execute(params![
                    book_id,
                    key,
                    word.count,
                    word.lemma_key,
                    word.is_name
                ])?;
            }
        }
        transaction.execute(
            "UPDATE book SET words_counted_with = ?2 WHERE id = ?1",
            params![book_id, version],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// A book's words against the saved ones in its language. All zero if the
    /// words were not counted yet.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn book_difficulty(&self, book_id: i64) -> Result<Difficulty, DbError> {
        let row = self.conn().query_row(
            // A word's status is its own, else its dictionary form's. Names
            // are left out unless saved on purpose.
            "SELECT COALESCE(SUM(w.count), 0),
                    COALESCE(SUM(CASE WHEN COALESCE(t.status, l.status) IS NULL THEN w.count END), 0),
                    COALESCE(SUM(CASE WHEN COALESCE(t.status, l.status) BETWEEN 1 AND 5 THEN w.count END), 0),
                    COUNT(DISTINCT COALESCE(w.lemma_key, w.key)),
                    COUNT(DISTINCT CASE WHEN COALESCE(t.status, l.status) IS NULL
                                        THEN COALESCE(w.lemma_key, w.key) END)
             FROM book_word w
             JOIN book b ON b.id = w.book_id
             LEFT JOIN term t ON t.language_code = b.language_code AND t.text = w.key
             LEFT JOIN term l ON l.language_code = b.language_code AND l.text = w.lemma_key
             WHERE w.book_id = ?1 AND (w.is_name = 0 OR t.status IS NOT NULL)",
            [book_id],
            |row| {
                Ok([
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                ])
            },
        )?;
        let [words, new_words, learning_words, unique_words, unique_new_words] =
            row.map(|n| u64::try_from(n).unwrap_or(0));
        Ok(Difficulty {
            words,
            new_words,
            learning_words,
            unique_words,
            unique_new_words,
        })
    }

    /// The words of a book taken for names, by term key. Empty until counted.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn book_names(&self, book_id: i64) -> Result<HashSet<String>, DbError> {
        let mut statement = self
            .conn()
            .prepare("SELECT key FROM book_word WHERE book_id = ?1 AND is_name = 1")?;
        let rows = statement.query_map([book_id], |row| row.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The book's forms and their dictionary forms' term keys ("lisait" →
    /// "lire"). Empty until counted.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn book_lemmas(&self, book_id: i64) -> Result<HashMap<String, String>, DbError> {
        let mut statement = self.conn().prepare(
            "SELECT key, lemma_key FROM book_word WHERE book_id = ?1 AND lemma_key IS NOT NULL",
        )?;
        let rows = statement.query_map([book_id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Marks a language's books for counting again, e.g. after its dictionary
    /// was installed or removed (names and forms come from it).
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn recount_books(&self, language_code: &str) -> Result<(), DbError> {
        self.conn().execute(
            "UPDATE book SET words_counted_with = 0 WHERE language_code = ?1",
            [language_code],
        )?;
        Ok(())
    }

    /// Rewrites today's saved-word counts for every language that has words
    /// or a history. Called after anything that saves, forgets or marks words.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn record_word_counts(&self) -> Result<(), DbError> {
        self.conn().execute(
            "INSERT INTO word_count_day (language_code, day, known, learning)
             SELECT l.code, date('now', 'localtime'),
                    (SELECT COUNT(*) FROM term t WHERE t.language_code = l.code AND t.status = 99),
                    (SELECT COUNT(*) FROM term t WHERE t.language_code = l.code AND t.status BETWEEN 1 AND 5)
             FROM language l
             WHERE l.code IN (SELECT language_code FROM term UNION SELECT language_code FROM word_count_day)
             ON CONFLICT (language_code, day) DO UPDATE SET
                 known = excluded.known, learning = excluded.learning",
            [],
        )?;
        Ok(())
    }

    /// The saved-word counts of a language, oldest day first. Days without
    /// changes have no row; the count stayed as on the day before.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn word_count_history(&self, language_code: &str) -> Result<Vec<DayCount>, DbError> {
        let mut statement = self.conn().prepare(
            "SELECT day, known, learning FROM word_count_day
             WHERE language_code = ?1 ORDER BY day",
        )?;
        let rows = statement.query_map([language_code], |row| {
            Ok(DayCount {
                day: row.get(0)?,
                known: row.get(1)?,
                learning: row.get(2)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::book::NewBook;
    use crate::db::term::{NewTerm, Status};
    use crate::db::tests_support::french_db;

    fn book(db: &Database) -> i64 {
        db.insert_book(&NewBook {
            title: "Un".into(),
            author: None,
            language_code: "fr".into(),
            file_path: "un.txt".into(),
            cover_path: None,
        })
        .unwrap()
    }

    fn term(text: &str, status: Status) -> NewTerm {
        NewTerm {
            language_code: "fr".into(),
            text: text.into(),
            lemma: None,
            status,
            translation: String::new(),
            notes: String::new(),
            parent_id: None,
            context: None,
        }
    }

    #[test]
    fn difficulty_counts_running_and_unique_words() {
        let db = french_db();
        let id = book(&db);
        assert_eq!(db.book_words_counted_with(id).unwrap(), None);
        let word = |count, lemma: Option<&str>, is_name| BookWord {
            count,
            lemma_key: lemma.map(str::to_string),
            is_name,
        };
        let words: HashMap<String, BookWord> = [
            ("le", word(6, None, false)),
            ("chat", word(2, None, false)),
            ("dort", word(1, None, false)),
            // A form of "dormir"; new while "dormir" is not saved.
            ("dormait", word(1, Some("dormir"), false)),
            // A name: left out.
            ("gregor", word(5, None, true)),
        ]
        .into_iter()
        .map(|(k, w)| (k.to_string(), w))
        .collect();
        db.store_book_words(id, 1, &words).unwrap();
        assert_eq!(db.book_words_counted_with(id).unwrap(), Some(1));
        db.save_term(&term("le", Status::WellKnown)).unwrap();
        db.save_term(&term("chat", Status::Learning(2))).unwrap();
        let difficulty = db.book_difficulty(id).unwrap();
        assert_eq!(
            difficulty,
            Difficulty {
                words: 10,
                new_words: 2,
                learning_words: 2,
                // le, chat, dort, dormir (for "dormait"); the name is left out.
                unique_words: 4,
                unique_new_words: 2,
            }
        );
        assert!((difficulty.new_percent() - 20.0).abs() < 1e-9);
        // Saving "dormir" makes its form "dormait" known too.
        db.save_term(&term("dormir", Status::WellKnown)).unwrap();
        assert_eq!(db.book_difficulty(id).unwrap().new_words, 1);
        assert_eq!(
            db.book_names(id).unwrap(),
            HashSet::from(["gregor".to_string()])
        );
        assert_eq!(db.book_lemmas(id).unwrap()["dormait"], "dormir");
        // A name saved on purpose counts again.
        db.save_term(&term("gregor", Status::Learning(1))).unwrap();
        assert_eq!(db.book_difficulty(id).unwrap().words, 15);
    }

    #[test]
    fn an_uncounted_book_is_all_zero_and_a_missing_one_not_found() {
        let db = french_db();
        let id = book(&db);
        assert_eq!(db.book_difficulty(id).unwrap(), Difficulty::default());
        assert!(matches!(
            db.book_words_counted_with(99),
            Err(DbError::NotFound { .. })
        ));
    }

    #[test]
    fn saving_words_records_today_s_counts() {
        let db = french_db();
        assert!(db.word_count_history("fr").unwrap().is_empty());
        db.save_term(&term("chat", Status::Learning(1))).unwrap();
        db.save_term(&term("le", Status::WellKnown)).unwrap();
        let history = db.word_count_history("fr").unwrap();
        assert_eq!(history.len(), 1);
        assert_eq!((history[0].known, history[0].learning), (1, 1));
        let chat = db.find_term("fr", "chat").unwrap().unwrap();
        db.delete_term(chat.id).unwrap();
        let history = db.word_count_history("fr").unwrap();
        assert_eq!((history[0].known, history[0].learning), (1, 0));
    }
}
