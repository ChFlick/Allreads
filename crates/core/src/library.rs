//! Importing books into the library and opening them for reading.
//!
//! Importing has two steps, so the database lock is held only briefly:
//! `prepare_import` reads the file (slow, no database), then `store_import`
//! writes the book, its chapter titles and its cover (fast).

use std::fs;
use std::path::Path;

use thiserror::Error;

use crate::book::{BookError, BookMeta};
use crate::db::book::NewBook;
use crate::db::{Database, DbError};
use crate::epub;
use crate::text::blocks::{Block, BlockKind, Span, Style};
use crate::txt::{self, TextBook};

/// A book file opened for reading, in either supported format.
pub enum OpenedBook {
    Epub(Box<epub::Book>),
    Text(TextBook),
}

impl OpenedBook {
    /// Opens an `.epub` or `.txt` file, chosen by its extension.
    ///
    /// # Errors
    /// Returns `NotFound` for a missing file, `Unsupported` for any other
    /// extension, and `Invalid` for a file that cannot be read.
    pub fn open(path: &Path) -> Result<Self, BookError> {
        if !path.is_file() {
            return Err(BookError::NotFound(path.to_path_buf()));
        }
        let extension = path
            .extension()
            .map(|ext| ext.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        match extension.as_str() {
            "epub" => Ok(Self::Epub(Box::new(epub::Book::open(path)?))),
            "txt" => Ok(Self::Text(TextBook::open(path)?)),
            other => Err(BookError::Unsupported(format!(".{other}"))),
        }
    }

    /// Title, author, language and chapter count. A text file has one chapter,
    /// and its title is the file name.
    #[must_use]
    pub fn meta(&self) -> BookMeta {
        match self {
            Self::Epub(book) => book.meta(),
            Self::Text(book) => BookMeta {
                title: Some(book.title().to_string()),
                author: None,
                language: None,
                chapters: 1,
            },
        }
    }

    /// One chapter's blocks. A text file's paragraphs become plain paragraph blocks.
    ///
    /// # Errors
    /// Returns `ChapterOutOfRange` if there is no such chapter.
    pub fn chapter_blocks(&self, index: usize) -> Result<Vec<Block>, BookError> {
        match self {
            Self::Epub(book) => book.chapter_blocks(index),
            Self::Text(book) if index == 0 => Ok(book
                .paragraphs()
                .iter()
                .map(|paragraph| Block {
                    kind: BlockKind::Paragraph,
                    spans: vec![Span {
                        text: paragraph.clone(),
                        style: Style::default(),
                    }],
                })
                .collect()),
            Self::Text(_) => Err(BookError::ChapterOutOfRange { index, count: 1 }),
        }
    }

    /// Each chapter's title, or `None` where a chapter has none.
    ///
    /// # Errors
    /// Returns `Invalid` if an EPUB chapter cannot be read.
    pub fn chapter_titles(&self) -> Result<Vec<Option<String>>, BookError> {
        match self {
            Self::Epub(book) => book.chapter_titles(),
            Self::Text(_) => Ok(vec![None]),
        }
    }

    /// The cover image and its file extension, if there is one.
    #[must_use]
    pub fn cover(&self) -> Option<(Vec<u8>, &'static str)> {
        match self {
            Self::Epub(book) => book.cover(),
            Self::Text(_) => None,
        }
    }
}

/// Errors from importing a book.
#[derive(Debug, Error)]
pub enum ImportError {
    #[error(transparent)]
    Book(#[from] BookError),
    #[error(transparent)]
    Db(#[from] DbError),
    #[error("could not save the cover: {0}")]
    Cover(String),
}

/// A book read from disk and ready to store.
pub struct PreparedImport {
    pub book: OpenedBook,
    pub file_path: String,
    pub title: String,
    pub author: Option<String>,
    pub chapter_titles: Vec<Option<String>>,
}

/// Reads a book file and everything the library needs from it. Touches no database.
///
/// # Errors
/// Returns a `BookError` if the file cannot be opened or read.
pub fn prepare_import(path: &Path) -> Result<PreparedImport, BookError> {
    let book = OpenedBook::open(path)?;
    let meta = book.meta();
    let title = meta
        .title
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
        .unwrap_or_else(|| txt::title_from_path(path));
    let chapter_titles = book.chapter_titles()?;
    Ok(PreparedImport {
        book,
        file_path: path.display().to_string(),
        title,
        author: meta.author,
        chapter_titles,
    })
}

/// Stores a prepared book and returns its id. The cover is written to
/// `covers_dir/<id>.<ext>`. If any step fails, nothing is left behind.
///
/// # Errors
/// Returns `Db` if the language is not stored or a statement fails, and
/// `Cover` if the cover cannot be written.
pub fn store_import(
    db: &Database,
    covers_dir: &Path,
    prepared: &PreparedImport,
    language_code: &str,
) -> Result<i64, ImportError> {
    let id = db.insert_book(&NewBook {
        title: prepared.title.clone(),
        author: prepared.author.clone(),
        language_code: language_code.to_string(),
        file_path: prepared.file_path.clone(),
        cover_path: None,
    })?;
    if let Err(error) = finish_import(db, covers_dir, prepared, id) {
        let _ = db.delete_book(id);
        return Err(error);
    }
    Ok(id)
}

fn finish_import(
    db: &Database,
    covers_dir: &Path,
    prepared: &PreparedImport,
    id: i64,
) -> Result<(), ImportError> {
    db.set_chapter_titles(id, &prepared.chapter_titles)?;
    let Some((bytes, extension)) = prepared.book.cover() else {
        return Ok(());
    };
    fs::create_dir_all(covers_dir).map_err(|e| ImportError::Cover(e.to_string()))?;
    let cover_path = covers_dir.join(format!("{id}.{extension}"));
    fs::write(&cover_path, bytes).map_err(|e| ImportError::Cover(e.to_string()))?;
    if let Err(error) = db.set_cover_path(id, Some(&cover_path.display().to_string())) {
        let _ = fs::remove_file(&cover_path);
        return Err(error.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presets::builtin_presets;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    fn library_db(path: &Path) -> Database {
        let db = Database::open(path).unwrap();
        for preset in builtin_presets().unwrap() {
            db.sync_builtin_language(&preset).unwrap();
        }
        db
    }

    fn import(db: &Database, covers: &Path, name: &str, language: &str) -> i64 {
        let prepared = prepare_import(&fixture(name)).unwrap();
        store_import(db, covers, &prepared, language).unwrap()
    }

    /// Step 9's check on the Rust side: books survive closing and reopening the library.
    #[test]
    fn imported_books_are_listed_after_reopening_the_library() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("allreads.db");
        let covers = dir.path().join("covers");
        {
            let db = library_db(&db_path);
            import(&db, &covers, "tiny.epub", "fr");
            import(&db, &covers, "layout.epub", "fr");
            import(&db, &covers, "sample.txt", "fr");
        }
        let db = library_db(&db_path);
        // Unread books are listed newest first; reverse to check them in import order.
        let mut books = db.books().unwrap();
        books.reverse();
        let titles: Vec<&str> = books.iter().map(|b| b.title.as_str()).collect();
        assert_eq!(titles, ["Le Petit Test", "Mise en page", "sample"]);
        assert!(books.iter().all(|b| b.language_code == "fr"));
        assert!(books
            .iter()
            .all(|b| (b.chapter_index, b.paragraph_index) == (0, 0)));
        assert_eq!(books[1].author.as_deref(), Some("Jules Auteur"));
        assert_eq!(
            db.chapter_titles(books[0].id).unwrap(),
            vec![
                Some("Chapitre un".to_string()),
                Some("Chapitre deux".to_string())
            ]
        );
        assert_eq!(db.chapter_titles(books[2].id).unwrap(), vec![None]);
    }

    #[test]
    fn cover_is_written_and_linked_only_for_books_that_have_one() {
        let dir = tempfile::tempdir().unwrap();
        let db = library_db(&dir.path().join("allreads.db"));
        let covers = dir.path().join("covers");
        let with_cover = import(&db, &covers, "layout.epub", "fr");
        let without = import(&db, &covers, "tiny.epub", "fr");
        let cover_path = db.book(with_cover).unwrap().unwrap().cover_path.unwrap();
        assert!(cover_path.ends_with(&format!("{with_cover}.png")));
        assert!(Path::new(&cover_path).is_file());
        assert_eq!(db.book(without).unwrap().unwrap().cover_path, None);
    }

    #[test]
    fn unknown_language_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let db = library_db(&dir.path().join("allreads.db"));
        let prepared = prepare_import(&fixture("layout.epub")).unwrap();
        let result = store_import(&db, &dir.path().join("covers"), &prepared, "zz");
        assert!(matches!(
            result,
            Err(ImportError::Db(DbError::NotFound { .. }))
        ));
        assert!(db.books().unwrap().is_empty());
        assert!(!dir.path().join("covers").exists());
    }

    #[test]
    fn unsupported_file_types_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.pdf");
        fs::write(&path, b"%PDF").unwrap();
        assert!(matches!(
            OpenedBook::open(&path),
            Err(BookError::Unsupported(ext)) if ext == ".pdf"
        ));
    }

    #[test]
    fn extension_check_ignores_case() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Livre.TXT");
        fs::write(&path, "Bonjour.").unwrap();
        assert!(matches!(OpenedBook::open(&path), Ok(OpenedBook::Text(_))));
    }

    #[test]
    fn text_book_has_exactly_one_chapter() {
        let book = OpenedBook::open(&fixture("sample.txt")).unwrap();
        assert_eq!(book.meta().chapters, 1);
        assert_eq!(book.chapter_blocks(0).unwrap().len(), 2);
        assert!(matches!(
            book.chapter_blocks(1),
            Err(BookError::ChapterOutOfRange { index: 1, count: 1 })
        ));
    }

    #[test]
    fn epub_title_falls_back_to_the_file_name_when_missing() {
        let prepared = prepare_import(&fixture("sample.txt")).unwrap();
        assert_eq!(prepared.title, "sample");
    }
}
