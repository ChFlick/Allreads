//! Catalogs of public-domain books, for finding something to read (plan:
//! "Find a book" in `IMPLEMENTATION_PLAN.md`).
//!
//! A catalog is a small `SQLite` file per language, built by a hand-run
//! workflow and downloaded like a dictionary. Besides each book's title,
//! author and subjects it holds the book's word counts, made with the same
//! counting as library books ([`crate::word_tally`]), so the app can say how
//! much of a book is new to the reader without downloading the book. Nothing
//! about the reader's words leaves the phone.

mod write;

use std::collections::{HashMap, HashSet};
use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use unicode_normalization::UnicodeNormalization;

use crate::db::progress::Difficulty;
use crate::db::term::Status;

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
CREATE TABLE book (
    -- The source's number for the book, e.g. Gutenberg's.
    id         INTEGER PRIMARY KEY,
    title      TEXT NOT NULL,
    author     TEXT NOT NULL,
    -- One subject per line.
    subjects   TEXT NOT NULL,
    -- Where the EPUB is downloaded from.
    url        TEXT NOT NULL,
    -- Downloads at the source, for sorting by popularity.
    popularity INTEGER NOT NULL,
    -- Running words, names left out. Rare words are counted here but have
    -- no row in book_word; they count as new.
    words      INTEGER NOT NULL
);
-- Every word once, by term key, with its dictionary form's id for a form
-- (\"lisait\" -> \"lire\").
CREATE TABLE word (
    id    INTEGER PRIMARY KEY,
    key   TEXT NOT NULL UNIQUE,
    lemma INTEGER REFERENCES word (id)
);
CREATE TABLE book_word (
    book_id INTEGER NOT NULL,
    word_id INTEGER NOT NULL,
    count   INTEGER NOT NULL,
    PRIMARY KEY (book_id, word_id)
) WITHOUT ROWID;
";

/// Why a catalog could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("catalog database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("not a usable catalog: {0}")]
    Format(String),
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

/// An open catalog. Its books are read once, when it is opened.
pub struct Catalog {
    conn: Connection,
    language_code: String,
    count_stamp: String,
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
        let books = read_books(&conn)?;
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
            language_code,
            count_stamp,
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
    /// Returns a database error if the catalog cannot be read.
    pub fn fit(&self, statuses: &HashMap<String, Status>) -> Result<Vec<BookFit>, CatalogError> {
        // Each word's status and the id of the word it is counted under (its
        // dictionary form's, if it has one).
        let mut words: HashMap<i64, (Option<Status>, i64)> = HashMap::new();
        let mut keys: HashMap<i64, String> = HashMap::new();
        let mut lemmas: HashMap<i64, i64> = HashMap::new();
        {
            let mut statement = self.conn.prepare("SELECT id, key, lemma FROM word")?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                let id: i64 = row.get(0)?;
                keys.insert(id, row.get(1)?);
                if let Some(lemma) = row.get::<_, Option<i64>>(2)? {
                    lemmas.insert(id, lemma);
                }
            }
        }
        for (&id, key) in &keys {
            let lemma = lemmas.get(&id).copied();
            let own = statuses.get(key).copied();
            let status = own.or_else(|| {
                lemma
                    .and_then(|l| keys.get(&l))
                    .and_then(|k| statuses.get(k).copied())
            });
            words.insert(id, (status, lemma.unwrap_or(id)));
        }

        let mut tallies: HashMap<i64, Tally> = HashMap::new();
        let mut statement = self
            .conn
            .prepare("SELECT book_id, word_id, count FROM book_word")?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let book_id: i64 = row.get(0)?;
            let word_id: i64 = row.get(1)?;
            let count: u32 = row.get(2)?;
            let (status, group) = words.get(&word_id).copied().unwrap_or((None, word_id));
            tallies
                .entry(book_id)
                .or_default()
                .add(status, group, count);
        }

        Ok(self
            .books
            .iter()
            .map(|book| {
                let tally = tallies.remove(&book.id).unwrap_or_default();
                tally.into_fit(book)
            })
            .collect())
    }
}

/// One book's counts while [`Catalog::fit`] reads its words.
#[derive(Default)]
struct Tally {
    words: u64,
    new_words: u64,
    learning_words: u64,
    groups: HashSet<i64>,
    /// Running count of each new word, by dictionary form.
    new_groups: HashMap<i64, u32>,
}

impl Tally {
    fn add(&mut self, status: Option<Status>, group: i64, count: u32) {
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

    fn into_fit(self, book: &CatalogBook) -> BookFit {
        // Rare words have no rows: they count as new, each its own word
        // (exact when only words met once are left out).
        let rare = book.words.saturating_sub(self.words);
        let worth_learning = self
            .new_groups
            .values()
            .filter(|&&count| count >= WORTH_LEARNING_COUNT)
            .count();
        BookFit {
            book_id: book.id,
            difficulty: Difficulty {
                words: self.words + rare,
                new_words: self.new_words + rare,
                learning_words: self.learning_words,
                unique_words: self.groups.len() as u64 + rare,
                unique_new_words: self.new_groups.len() as u64 + rare,
            },
            worth_learning: u32::try_from(worth_learning).unwrap_or(u32::MAX),
        }
    }
}

fn read_books(conn: &Connection) -> Result<Vec<CatalogBook>, CatalogError> {
    let mut statement = conn.prepare(
        "SELECT id, title, author, subjects, url, popularity, words FROM book
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
            // Stored as SQLite's signed integer; never negative.
            words: u64::try_from(row.get::<_, i64>(6)?).unwrap_or(0),
        })
    })?;
    Ok(rows.collect::<Result<_, _>>()?)
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
