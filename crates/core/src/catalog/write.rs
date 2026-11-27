//! Writing a catalog: used by the catalog builder, never by the app.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{params, Connection};

use super::pack::pack;
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
    /// When it was first published, if known.
    pub year: Option<i32>,
}

/// A book waiting for [`CatalogWriter::finish`], its words by index into
/// [`CatalogWriter::keys`].
struct PendingBook {
    book: NewCatalogBook,
    words: u64,
    counts: Vec<(usize, u32)>,
}

/// Builds a catalog file. Books are collected in memory and written by
/// [`CatalogWriter::finish`], which numbers the words by how common they are
/// across the whole catalog, so the packed counts stay small.
pub struct CatalogWriter {
    conn: Connection,
    books: Vec<PendingBook>,
    book_ids: HashSet<i64>,
    /// Every word's term key, in the order first met.
    keys: Vec<String>,
    /// The index of each key in `keys`.
    indexes: HashMap<String, usize>,
    /// The dictionary form of a form, by index ("lisait" → "lire").
    lemmas: HashMap<usize, usize>,
    /// Each word's running count over all books, by index.
    totals: Vec<u64>,
}

impl CatalogWriter {
    /// Creates a new catalog at `path` for books in `language_code`, counted
    /// with `count_stamp` (see [`crate::word_tally::count_stamp`]) and built
    /// on `built` (a date, e.g. `2026-12-01`).
    ///
    /// # Errors
    /// Returns `Format` if a file already exists at `path`, and `Sql` if it
    /// cannot be created.
    pub fn create(
        path: &Path,
        language_code: &str,
        count_stamp: &str,
        built: &str,
    ) -> Result<Self, CatalogError> {
        if path.exists() {
            return Err(CatalogError::Format(format!(
                "{} already exists",
                path.display()
            )));
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        for (key, value) in [
            ("format", FORMAT_VERSION.to_string()),
            ("language", language_code.to_string()),
            ("count_stamp", count_stamp.to_string()),
            ("built", built.to_string()),
        ] {
            conn.execute(
                "INSERT INTO info (key, value) VALUES (?1, ?2)",
                params![key, value],
            )?;
        }
        Ok(Self {
            conn,
            books: Vec::new(),
            book_ids: HashSet::new(),
            keys: Vec::new(),
            indexes: HashMap::new(),
            lemmas: HashMap::new(),
            totals: Vec::new(),
        })
    }

    /// Adds a book and its words, as counted by [`crate::word_tally::count_book`].
    /// Names are left out, as in a library book's difficulty.
    ///
    /// # Errors
    /// Returns `Format` for a book id already added.
    pub fn add_book(
        &mut self,
        book: &NewCatalogBook,
        words: &HashMap<String, BookWord>,
    ) -> Result<(), CatalogError> {
        if !self.book_ids.insert(book.id) {
            return Err(CatalogError::Format(format!(
                "book {} added twice",
                book.id
            )));
        }
        let mut running: u64 = 0;
        let mut counts = Vec::new();
        for (key, word) in words.iter().filter(|(_, word)| !word.is_name) {
            running += u64::from(word.count);
            let index = self.index(key);
            if let Some(lemma_key) = &word.lemma_key {
                let lemma = self.index(lemma_key);
                self.lemmas.entry(index).or_insert(lemma);
            }
            self.totals[index] += u64::from(word.count);
            counts.push((index, word.count));
        }
        self.books.push(PendingBook {
            book: book.clone(),
            words: running,
            counts,
        });
        Ok(())
    }

    /// Writes the words and books and compacts the file, so the download is
    /// as small as it can be.
    ///
    /// # Errors
    /// Returns a database error if writing fails.
    pub fn finish(mut self) -> Result<(), CatalogError> {
        // Word ids by how common a word is: the most common gets 0. Ties go by
        // key, so the same books always give the same file.
        let mut order: Vec<usize> = (0..self.keys.len()).collect();
        order.sort_by(|&a, &b| {
            self.totals[b]
                .cmp(&self.totals[a])
                .then_with(|| self.keys[a].cmp(&self.keys[b]))
        });
        let mut ids = vec![0_u32; self.keys.len()];
        for (id, &index) in order.iter().enumerate() {
            ids[index] =
                u32::try_from(id).map_err(|_| CatalogError::Format("too many words".into()))?;
        }

        let transaction = self.conn.transaction()?;
        {
            let mut insert =
                transaction.prepare("INSERT INTO word (id, key, lemma) VALUES (?1, ?2, ?3)")?;
            for &index in &order {
                let lemma = self.lemmas.get(&index).map(|&l| ids[l]);
                insert.execute(params![ids[index], self.keys[index], lemma])?;
            }
            let mut insert = transaction.prepare(
                "INSERT INTO book (id, title, author, subjects, url, popularity, year, words, counts)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            )?;
            for pending in &self.books {
                let mut counts: Vec<(u32, u32)> = pending
                    .counts
                    .iter()
                    .map(|&(index, count)| (ids[index], count))
                    .collect();
                counts.sort_unstable();
                let book = &pending.book;
                insert.execute(params![
                    book.id,
                    book.title,
                    book.author,
                    book.subjects.join("\n"),
                    book.url,
                    book.popularity,
                    book.year,
                    i64::try_from(pending.words).unwrap_or(i64::MAX),
                    pack(&counts),
                ])?;
            }
        }
        transaction.commit()?;
        self.conn.execute_batch("VACUUM;")?;
        Ok(())
    }

    /// The index of `key` in `keys`, adding it first if needed.
    fn index(&mut self, key: &str) -> usize {
        if let Some(&index) = self.indexes.get(key) {
            return index;
        }
        let index = self.keys.len();
        self.keys.push(key.to_string());
        self.indexes.insert(key.to_string(), index);
        self.totals.push(0);
        index
    }
}
