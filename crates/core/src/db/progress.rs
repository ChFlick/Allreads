//! Progress: how hard a book is (its share of new words), and how many words
//! are known per day.

use std::collections::{HashMap, HashSet};

use rusqlite::params;

use super::{Database, DbError};
use crate::db::term::Status;
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

/// A counted book's difficulty and the stamp it was counted with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CountedDifficulty {
    pub book_id: i64,
    pub language_code: String,
    pub stamp: String,
    pub difficulty: Difficulty,
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
    /// The stamp a book's words were counted with (see
    /// [`crate::word_tally::count_stamp`]), or `None` if not counted.
    ///
    /// # Errors
    /// Returns `NotFound` if the book does not exist.
    pub fn book_count_stamp(&self, book_id: i64) -> Result<Option<String>, DbError> {
        use rusqlite::OptionalExtension;
        let stamp: Option<String> = self
            .conn()
            .query_row(
                "SELECT words_counted FROM book WHERE id = ?1",
                [book_id],
                |row| row.get(0),
            )
            .optional()?;
        match stamp {
            None => Err(DbError::NotFound {
                what: format!("book {book_id}"),
            }),
            Some(stamp) if stamp.is_empty() => Ok(None),
            Some(stamp) => Ok(Some(stamp)),
        }
    }

    /// Replaces a book's counted words (term key → count, form, name).
    /// `stamp` says which rules and dictionary made them.
    ///
    /// # Errors
    /// Returns a database error if a statement fails.
    pub fn store_book_words(
        &self,
        book_id: i64,
        stamp: &str,
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
            "UPDATE book SET words_counted = ?2 WHERE id = ?1",
            params![book_id, stamp],
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
        let found = self.difficulties("w.book_id = ?1", book_id)?;
        Ok(found.get(&book_id).copied().unwrap_or_default())
    }

    /// The difficulty of every counted book, by book id, in one query, with
    /// the stamp it was counted with: the caller keeps those whose stamp is
    /// still today's for their language.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn counted_difficulties(&self) -> Result<Vec<CountedDifficulty>, DbError> {
        let difficulties = self.difficulties("b.words_counted <> ?1", "")?;
        let mut statement = self.conn().prepare(
            "SELECT id, language_code, words_counted FROM book WHERE words_counted <> ''",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get(1)?, row.get(2)?))
        })?;
        let mut counted = Vec::new();
        for row in rows {
            let (book_id, language_code, stamp) = row?;
            counted.push(CountedDifficulty {
                book_id,
                language_code,
                stamp,
                // A counted book without words has no row in the query above.
                difficulty: difficulties.get(&book_id).copied().unwrap_or_default(),
            });
        }
        Ok(counted)
    }

    /// Difficulties per book for the books matching `filter`, an SQL condition
    /// that may use `?1` for `value`.
    fn difficulties(
        &self,
        filter: &str,
        value: impl rusqlite::ToSql,
    ) -> Result<HashMap<i64, Difficulty>, DbError> {
        // A word's status is its own, else its dictionary form's. Names are
        // left out unless saved on purpose.
        let sql = format!(
            "SELECT w.book_id,
                    SUM(w.count),
                    COALESCE(SUM(CASE WHEN COALESCE(t.status, l.status) IS NULL THEN w.count END), 0),
                    COALESCE(SUM(CASE WHEN COALESCE(t.status, l.status) BETWEEN ?2 AND ?3 THEN w.count END), 0),
                    COUNT(DISTINCT COALESCE(w.lemma_key, w.key)),
                    COUNT(DISTINCT CASE WHEN COALESCE(t.status, l.status) IS NULL
                                        THEN COALESCE(w.lemma_key, w.key) END)
             FROM book_word w
             JOIN book b ON b.id = w.book_id
             LEFT JOIN term t ON t.language_code = b.language_code AND t.text = w.key
             LEFT JOIN term l ON l.language_code = b.language_code AND l.text = w.lemma_key
             WHERE {filter} AND (w.is_name = 0 OR t.status IS NOT NULL)
             GROUP BY w.book_id"
        );
        let mut statement = self.conn().prepare_cached(&sql)?;
        let rows = statement.query_map(
            params![
                value,
                Status::Learning(1).code(),
                Status::Learning(5).code()
            ],
            |row| {
                let number = |i| row.get::<_, i64>(i).map(|n| u64::try_from(n).unwrap_or(0));
                Ok((
                    row.get::<_, i64>(0)?,
                    Difficulty {
                        words: number(1)?,
                        new_words: number(2)?,
                        learning_words: number(3)?,
                        unique_words: number(4)?,
                        unique_new_words: number(5)?,
                    },
                ))
            },
        )?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The languages of the books and of the saved words: the language of the
    /// book read most recently first, then by code. For the Words tab, so
    /// words stay reachable after their book is deleted.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn word_languages(&self) -> Result<Vec<String>, DbError> {
        let mut statement = self.conn().prepare(
            "SELECT language_code FROM (
                 SELECT language_code, COALESCE(MAX(last_read_at), 0) AS read FROM book GROUP BY language_code
                 UNION ALL
                 SELECT DISTINCT language_code, -1 FROM term)
             GROUP BY language_code
             ORDER BY MAX(read) DESC, language_code",
        )?;
        let rows = statement.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<Result<_, _>>()?)
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

    /// Rewrites today's saved-word counts for one language, or for every
    /// language with words or a history when `language_code` is `None` (after
    /// a change that may touch several). Runs inside the caller's transaction
    /// when there is one.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn record_word_counts(&self, language_code: Option<&str>) -> Result<(), DbError> {
        self.conn().execute(
            "INSERT INTO word_count_day (language_code, day, known, learning)
             SELECT l.code, date('now', 'localtime'),
                    (SELECT COUNT(*) FROM term t WHERE t.language_code = l.code AND t.status = ?2),
                    (SELECT COUNT(*) FROM term t WHERE t.language_code = l.code AND t.status BETWEEN ?3 AND ?4)
             FROM language l
             WHERE (?1 IS NULL OR l.code = ?1)
               AND l.code IN (SELECT language_code FROM term UNION SELECT language_code FROM word_count_day)
             ON CONFLICT (language_code, day) DO UPDATE SET
                 known = excluded.known, learning = excluded.learning",
            params![
                language_code,
                Status::WellKnown.code(),
                Status::Learning(1).code(),
                Status::Learning(5).code()
            ],
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
        assert_eq!(db.book_count_stamp(id).unwrap(), None);
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
        db.store_book_words(id, "1.3/", &words).unwrap();
        assert_eq!(db.book_count_stamp(id).unwrap().as_deref(), Some("1.3/"));
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
        let counted = db.counted_difficulties().unwrap();
        assert_eq!(
            (counted[0].book_id, counted[0].stamp.as_str()),
            (id, "1.3/")
        );
        assert_eq!(counted[0].difficulty, db.book_difficulty(id).unwrap());
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
            db.book_count_stamp(99),
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
        assert_eq!(db.word_languages().unwrap(), ["fr"]);
        let chat = db.find_term("fr", "chat").unwrap().unwrap();
        db.delete_term(chat.id).unwrap();
        let history = db.word_count_history("fr").unwrap();
        assert_eq!((history[0].known, history[0].learning), (1, 0));
    }
}
