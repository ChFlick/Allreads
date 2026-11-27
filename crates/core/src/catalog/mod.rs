//! Catalogs of public-domain books, for finding something to read (plan:
//! "Find a book" in `IMPLEMENTATION_PLAN.md`).
//!
//! A catalog is a small `SQLite` file per language, built by a hand-run
//! workflow and downloaded like a dictionary. Besides each book's title,
//! author and subjects it holds the book's word counts, made with the same
//! counting as library books ([`crate::word_tally`]), so the app can say how
//! much of a book is new to the reader without downloading the book. Nothing
//! about the reader's words leaves the phone.

mod install;
mod pack;
mod write;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use unicode_normalization::UnicodeNormalization;

use crate::db::progress::Difficulty;
use crate::db::term::Status;

pub use install::{
    install, installed, installed_path, parse_catalog_manifest, remove, valid_language,
    AvailableCatalog, InstalledCatalog,
};
pub use write::{CatalogWriter, NewCatalogBook};

/// The catalog file format. A catalog of another version is refused, so the
/// app asks for a newer download instead of misreading it.
pub const FORMAT_VERSION: u32 = 1;

/// A new word that occurs at least this often in a book is "worth learning":
/// learning it pays off within the book.
pub const WORTH_LEARNING_COUNT: u32 = 5;

const SCHEMA: &str = "
CREATE TABLE info (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
-- Every word once, by term key. Ids follow how common a word is across the
-- catalog (0 is the most common), so packed counts stay small. A form links
-- to its dictionary form (\"lisait\" -> \"lire\").
CREATE TABLE word (
    id    INTEGER PRIMARY KEY,
    -- Unique (the writer makes sure); no index, since the app never looks a key up.
    key   TEXT NOT NULL,
    lemma INTEGER
);
CREATE TABLE book (
    -- The source's number for the book, e.g. Gutenberg's.
    id         INTEGER PRIMARY KEY,
    title      TEXT NOT NULL,
    author     TEXT NOT NULL,
    -- One subject per line.
    subjects   TEXT NOT NULL,
    -- Where the EPUB is downloaded from.
    url        TEXT NOT NULL,
    -- How popular the book is at its source, for sorting.
    popularity INTEGER NOT NULL,
    -- First published, if known.
    year       INTEGER,
    -- Running words, names left out.
    words      INTEGER NOT NULL,
    -- The book's (word id, count) pairs, packed (see pack.rs).
    counts     BLOB NOT NULL
);
";

/// Why a catalog could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("catalog database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("not a usable catalog: {0}")]
    Format(String),
    #[error("the catalog download is damaged")]
    ChecksumMismatch,
    #[error("catalog file error: {0}")]
    Io(#[from] std::io::Error),
}

/// A book in a catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogBook {
    pub id: i64,
    pub title: String,
    pub author: String,
    pub subjects: Vec<String>,
    pub url: String,
    pub popularity: u32,
    /// First published, if known.
    pub year: Option<i32>,
    /// Running words, names left out.
    pub words: u64,
}

/// How well a catalog book fits the reader's saved words.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BookFit {
    pub book_id: i64,
    /// The same numbers as a library book's difficulty.
    pub difficulty: Difficulty,
    /// New words (by dictionary form) that occur at least
    /// [`WORTH_LEARNING_COUNT`] times in the book.
    pub worth_learning: u32,
}

/// A word in the catalog's word list.
struct CatalogWord {
    key: String,
    /// The id of its dictionary form, for a form.
    lemma: Option<u32>,
}

/// An open catalog. Its books and word list are read once, when it is opened;
/// the packed counts are read when they are needed.
pub struct Catalog {
    conn: Connection,
    words: Vec<CatalogWord>,
    language_code: String,
    count_stamp: String,
    built: String,
    books: Vec<CatalogBook>,
    /// Each book's title, author and subjects for searching: lower case, no accents.
    search_text: Vec<String>,
}

impl Catalog {
    /// Opens a catalog file read-only.
    ///
    /// # Errors
    /// Returns `Format` for a file that is not a catalog or has another
    /// [`FORMAT_VERSION`], and `Sql` if it cannot be read.
    pub fn open(path: &Path) -> Result<Self, CatalogError> {
        let conn = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        let info = |key: &str| -> Result<String, CatalogError> {
            conn.query_row("SELECT value FROM info WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|_| CatalogError::Format("no catalog information".into()))?
            .ok_or_else(|| CatalogError::Format(format!("no {key}")))
        };
        let version = info("format")?;
        if version != FORMAT_VERSION.to_string() {
            return Err(CatalogError::Format(format!(
                "format {version}, expected {FORMAT_VERSION}"
            )));
        }
        let language_code = info("language")?;
        let count_stamp = info("count_stamp")?;
        let built = info("built")?;
        let books = read_books(&conn)?;
        let words = read_words(&conn)?;
        let search_text = books
            .iter()
            .map(|book| {
                search_form(&format!(
                    "{} {} {}",
                    book.title,
                    book.author,
                    book.subjects.join(" ")
                ))
            })
            .collect();
        Ok(Self {
            conn,
            words,
            language_code,
            count_stamp,
            built,
            books,
            search_text,
        })
    }

    /// The language of the catalog's books.
    #[must_use]
    pub fn language_code(&self) -> &str {
        &self.language_code
    }

    /// The counting rules and dictionary the words were counted with (see
    /// [`crate::word_tally::count_stamp`]).
    #[must_use]
    pub fn count_stamp(&self) -> &str {
        &self.count_stamp
    }

    /// When the catalog was built, e.g. `2026-12-01`.
    #[must_use]
    pub fn built(&self) -> &str {
        &self.built
    }

    /// Every book, most popular first.
    #[must_use]
    pub fn books(&self) -> &[CatalogBook] {
        &self.books
    }

    /// The books whose title, author or subjects contain every word of
    /// `query`, ignoring case and accents ("miserables hugo"). An empty query
    /// finds every book. Most popular first.
    #[must_use]
    pub fn search(&self, query: &str) -> Vec<&CatalogBook> {
        let query = search_form(query);
        let words: Vec<&str> = query.split_whitespace().collect();
        self.books
            .iter()
            .zip(&self.search_text)
            .filter(|(_, text)| words.iter().all(|word| text.contains(word)))
            .map(|(book, _)| book)
            .collect()
    }

    /// How well every book fits the saved words, by term key (from
    /// [`crate::db::Database::term_statuses`]), in the order of [`Catalog::books`].
    /// A word counts as known or learning by its own status, else by its
    /// dictionary form's, as for library books.
    ///
    /// # Errors
    /// Returns `Format` for damaged counts and `Sql` if the catalog cannot be read.
    pub fn fit(&self, statuses: &HashMap<String, Status>) -> Result<Vec<BookFit>, CatalogError> {
        let states = self.word_states(statuses);
        let mut fits = Vec::with_capacity(self.books.len());
        for book in &self.books {
            let mut tally = Tally::default();
            for (id, count) in self.counts(book.id)? {
                let (status, group) = states.get(id as usize).copied().unwrap_or((None, id));
                tally.add(status, group, count);
            }
            fits.push(tally.into_fit(book.id));
        }
        Ok(fits)
    }

    /// The new words of a book that occur most often, by dictionary form, with
    /// how often they occur: what reading it would teach. At most `limit`.
    ///
    /// # Errors
    /// Returns `Format` for an unknown book or damaged counts, and `Sql` if
    /// the catalog cannot be read.
    pub fn top_new_words(
        &self,
        book_id: i64,
        statuses: &HashMap<String, Status>,
        limit: usize,
    ) -> Result<Vec<(String, u32)>, CatalogError> {
        let states = self.word_states(statuses);
        let mut new_groups: HashMap<u32, u32> = HashMap::new();
        for (id, count) in self.counts(book_id)? {
            let (status, group) = states.get(id as usize).copied().unwrap_or((None, id));
            if status.is_none() {
                *new_groups.entry(group).or_default() += count;
            }
        }
        let mut top: Vec<(u32, u32)> = new_groups.into_iter().collect();
        // Most frequent first; ties by word id, i.e. the more common word first.
        top.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        Ok(top
            .into_iter()
            .take(limit)
            .filter_map(|(id, count)| self.words.get(id as usize).map(|w| (w.key.clone(), count)))
            .collect())
    }

    /// Each word's status (its own, else its dictionary form's) and the id it
    /// is counted under (its dictionary form's, if it has one), by word id.
    fn word_states(&self, statuses: &HashMap<String, Status>) -> Vec<(Option<Status>, u32)> {
        self.words
            .iter()
            .enumerate()
            .map(|(id, word)| {
                let id = u32::try_from(id).unwrap_or(u32::MAX);
                let lemma_key = word
                    .lemma
                    .and_then(|l| self.words.get(l as usize))
                    .map(|l| &l.key);
                let status = statuses
                    .get(&word.key)
                    .or_else(|| lemma_key.and_then(|k| statuses.get(k)))
                    .copied();
                (status, word.lemma.unwrap_or(id))
            })
            .collect()
    }

    /// A book's (word id, count) pairs.
    fn counts(&self, book_id: i64) -> Result<Vec<(u32, u32)>, CatalogError> {
        let bytes: Vec<u8> = self
            .conn
            .prepare_cached("SELECT counts FROM book WHERE id = ?1")?
            .query_row([book_id], |row| row.get(0))
            .optional()?
            .ok_or_else(|| CatalogError::Format(format!("no book {book_id}")))?;
        pack::unpack(&bytes)
    }
}

/// One book's counts while [`Catalog::fit`] reads its words.
#[derive(Default)]
struct Tally {
    words: u64,
    new_words: u64,
    learning_words: u64,
    /// The dictionary forms met.
    groups: HashSet<u32>,
    /// Running count of each new word, by dictionary form.
    new_groups: HashMap<u32, u32>,
}

impl Tally {
    fn add(&mut self, status: Option<Status>, group: u32, count: u32) {
        self.words += u64::from(count);
        self.groups.insert(group);
        match status {
            None => {
                self.new_words += u64::from(count);
                *self.new_groups.entry(group).or_default() += count;
            }
            Some(Status::Learning(_)) => self.learning_words += u64::from(count),
            Some(Status::WellKnown | Status::Ignored) => {}
        }
    }

    fn into_fit(self, book_id: i64) -> BookFit {
        let worth_learning = self
            .new_groups
            .values()
            .filter(|&&count| count >= WORTH_LEARNING_COUNT)
            .count();
        BookFit {
            book_id,
            difficulty: Difficulty {
                words: self.words,
                new_words: self.new_words,
                learning_words: self.learning_words,
                unique_words: self.groups.len() as u64,
                unique_new_words: self.new_groups.len() as u64,
            },
            worth_learning: u32::try_from(worth_learning).unwrap_or(u32::MAX),
        }
    }
}

fn read_books(conn: &Connection) -> Result<Vec<CatalogBook>, CatalogError> {
    let mut statement = conn.prepare(
        "SELECT id, title, author, subjects, url, popularity, year, words FROM book
         ORDER BY popularity DESC, title",
    )?;
    let rows = statement.query_map([], |row| {
        let subjects: String = row.get(3)?;
        Ok(CatalogBook {
            id: row.get(0)?,
            title: row.get(1)?,
            author: row.get(2)?,
            subjects: subjects.lines().map(str::to_string).collect(),
            url: row.get(4)?,
            popularity: row.get(5)?,
            year: row.get(6)?,
            // Stored as SQLite's signed integer; never negative.
            words: u64::try_from(row.get::<_, i64>(7)?).unwrap_or(0),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// The word list, by id. Ids run from 0 without gaps (see the writer).
fn read_words(conn: &Connection) -> Result<Vec<CatalogWord>, CatalogError> {
    let mut statement = conn.prepare("SELECT id, key, lemma FROM word ORDER BY id")?;
    let mut rows = statement.query([])?;
    let mut words = Vec::new();
    while let Some(row) = rows.next()? {
        let id: i64 = row.get(0)?;
        if id != words.len() as i64 {
            return Err(CatalogError::Format("gaps in the word list".into()));
        }
        words.push(CatalogWord {
            key: row.get(1)?,
            lemma: row.get(2)?,
        });
    }
    Ok(words)
}

/// Text for searching: lower case, without accents ("Misérables" → "miserables").
fn search_form(text: &str) -> String {
    text.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
}

#[cfg(test)]
mod tests;
