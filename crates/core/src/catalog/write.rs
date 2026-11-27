//! Writing a catalog: used by the catalog builder, never by the app.

use std::collections::HashMap;
use std::path::Path;

use rusqlite::{params, Connection};

use super::{CatalogError, FORMAT_VERSION, SCHEMA};
use crate::word_tally::BookWord;

/// A book to add to a catalog, without its words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewCatalogBook {
    /// The source's number for the book, e.g. Gutenberg's.
    pub id: i64,
    pub title: String,
    pub author: String,
    pub subjects: Vec<String>,
    pub url: String,
    pub popularity: u32,
}

/// Builds a catalog file one book at a time.
pub struct CatalogWriter {
    conn: Connection,
    /// Word ids by term key, so each word is stored once.
    word_ids: HashMap<String, i64>,
    min_count: u32,
}

impl CatalogWriter {
    /// Creates a new catalog at `path` for books in `language_code`, counted
    /// with `count_stamp` (see [`crate::word_tally::count_stamp`]). Words that
    /// occur fewer than `min_count` times in a book get no row of their own:
    /// they only add to the book's running words and count as new. A
    /// `min_count` of 2 leaves out the words met once, about half of a
    /// novel's different words but only a few percent of its running words.
    ///
    /// # Errors
    /// Returns `Format` if a file already exists at `path`, and `Sql` if it
    /// cannot be created.
    pub fn create(
        path: &Path,
        language_code: &str,
        count_stamp: &str,
        min_count: u32,
    ) -> Result<Self, CatalogError> {
        if path.exists() {
            return Err(CatalogError::Format(format!(
                "{} already exists",
                path.display()
            )));
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        let mut insert = conn.prepare("INSERT INTO info (key, value) VALUES (?1, ?2)")?;
        for (key, value) in [
            ("format", FORMAT_VERSION.to_string()),
            ("language", language_code.to_string()),
            ("count_stamp", count_stamp.to_string()),
            ("min_count", min_count.to_string()),
        ] {
            insert.execute(params![key, value])?;
        }
        drop(insert);
        Ok(Self {
            conn,
            word_ids: HashMap::new(),
            min_count,
        })
    }

    /// Adds a book and its words, as counted by [`crate::word_tally::count_book`].
    /// Names are left out, as in a library book's difficulty.
    ///
    /// # Errors
    /// Returns a database error, e.g. for a book id already in the catalog.
    pub fn add_book(
        &mut self,
        book: &NewCatalogBook,
        words: &HashMap<String, BookWord>,
    ) -> Result<(), CatalogError> {
        let transaction = self.conn.transaction()?;
        let mut running: i64 = 0;
        let mut kept: Vec<(i64, u32)> = Vec::new();
        // Sorted, so the same book always gives the same file.
        let mut sorted: Vec<(&String, &BookWord)> =
            words.iter().filter(|(_, word)| !word.is_name).collect();
        sorted.sort_by(|a, b| a.0.cmp(b.0));
        for (key, word) in sorted {
            running += i64::from(word.count);
            if word.count < self.min_count {
                continue;
            }
            let lemma = match &word.lemma_key {
                Some(lemma_key) => {
                    Some(word_id(&transaction, &mut self.word_ids, lemma_key, None)?)
                }
                None => None,
            };
            kept.push((
                word_id(&transaction, &mut self.word_ids, key, lemma)?,
                word.count,
            ));
        }
        transaction.execute(
            "INSERT INTO book (id, title, author, subjects, url, popularity, words)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                book.id,
                book.title,
                book.author,
                book.subjects.join("\n"),
                book.url,
                book.popularity,
                running
            ],
        )?;
        {
            let mut insert = transaction
                .prepare("INSERT INTO book_word (book_id, word_id, count) VALUES (?1, ?2, ?3)")?;
            for (word, count) in kept {
                insert.execute(params![book.id, word, count])?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    /// Finishes the file: compacts it, so the download is as small as it can be.
    ///
    /// # Errors
    /// Returns a database error if compacting fails.
    pub fn finish(self) -> Result<(), CatalogError> {
        self.conn.execute_batch("ANALYZE; VACUUM;")?;
        Ok(())
    }
}

/// The id of the word `key`, adding it first if needed. A word met first as a
/// dictionary form gets its own lemma link when it is later met as a form.
fn word_id(
    conn: &Connection,
    ids: &mut HashMap<String, i64>,
    key: &str,
    lemma: Option<i64>,
) -> Result<i64, CatalogError> {
    if let Some(&id) = ids.get(key) {
        if lemma.is_some() {
            conn.execute(
                "UPDATE word SET lemma = ?2 WHERE id = ?1 AND lemma IS NULL",
                params![id, lemma],
            )?;
        }
        return Ok(id);
    }
    conn.execute(
        "INSERT INTO word (key, lemma) VALUES (?1, ?2)",
        params![key, lemma],
    )?;
    let id = conn.last_insert_rowid();
    ids.insert(key.to_string(), id);
    Ok(id)
}
