//! Thin `UniFFI` wrappers over `allreads-core`. Kept deliberately small:
//! a few coarse calls that return plain records, never per-word calls.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use allreads_core::db::book::Book as BookRow;
use allreads_core::db::{Database, DbError};
use allreads_core::epub::BookError;
use allreads_core::library::{self, ImportError, OpenedBook};
use allreads_core::presets::builtin_presets;

uniffi::setup_scaffolding!();

/// Returns the core version.
#[uniffi::export]
#[must_use]
pub fn core_version() -> String {
    allreads_core::version().to_string()
}

/// Facts read from a book file before it is imported, e.g. to suggest its language.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BookMeta {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub chapters: u32,
}

impl From<allreads_core::epub::BookMeta> for BookMeta {
    fn from(meta: allreads_core::epub::BookMeta) -> Self {
        BookMeta {
            title: meta.title,
            author: meta.author,
            language: meta.language,
            chapters: saturate_u32(meta.chapters),
        }
    }
}

/// One book on the library screen.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BookSummary {
    pub id: i64,
    pub title: String,
    pub author: Option<String>,
    pub language_code: String,
    pub cover_path: Option<String>,
    pub chapter_count: u32,
    /// Where the reader stopped.
    pub chapter_index: u32,
    pub paragraph_index: u32,
}

/// Converts a count to `u32`, clamping instead of wrapping.
fn saturate_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// Errors that cross into Kotlin as typed exceptions.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum AllreadsError {
    #[error("file not found: {path}")]
    NotFound { path: String },
    #[error("could not read the book: {message}")]
    InvalidBook { message: String },
    #[error("unsupported file type: {message}")]
    UnsupportedFile { message: String },
    #[error("chapter {index} is out of range; the book has {count} chapters")]
    ChapterOutOfRange { index: u32, count: u32 },
    #[error("data folder not found: {path}")]
    DataFolderMissing { path: String },
    /// A book, language or other row that does not exist.
    #[error("{what} not found")]
    Missing { what: String },
    #[error("could not write a file: {message}")]
    Storage { message: String },
    #[error("database error: {message}")]
    Database { message: String },
}

impl From<DbError> for AllreadsError {
    fn from(error: DbError) -> Self {
        match error {
            DbError::NotFound { what } => AllreadsError::Missing { what },
            other => AllreadsError::Database {
                message: other.to_string(),
            },
        }
    }
}

impl From<BookError> for AllreadsError {
    fn from(error: BookError) -> Self {
        match error {
            BookError::NotFound(path) => AllreadsError::NotFound {
                path: path.display().to_string(),
            },
            BookError::Invalid(message) => AllreadsError::InvalidBook { message },
            BookError::Unsupported(message) => AllreadsError::UnsupportedFile { message },
            BookError::ChapterOutOfRange { index, count } => AllreadsError::ChapterOutOfRange {
                index: saturate_u32(index),
                count: saturate_u32(count),
            },
        }
    }
}

impl From<ImportError> for AllreadsError {
    fn from(error: ImportError) -> Self {
        match error {
            ImportError::Book(error) => error.into(),
            ImportError::Db(error) => error.into(),
            ImportError::Cover(message) => AllreadsError::Storage { message },
        }
    }
}

fn poisoned() -> AllreadsError {
    AllreadsError::Database {
        message: "a lock is poisoned after an earlier crash".into(),
    }
}

/// The app's library: one `SQLite` database and the books opened from it.
///
/// Create it once and keep it for the life of the app. Kotlin calls it on
/// `Dispatchers.IO`, so shared state sits behind a `Mutex`.
#[derive(uniffi::Object)]
pub struct App {
    db: Mutex<Database>,
    data_dir: PathBuf,
    /// Books opened since the app started, so reading a chapter does not re-parse the file.
    open_books: Mutex<HashMap<i64, Arc<OpenedBook>>>,
}

impl App {
    fn db(&self) -> Result<MutexGuard<'_, Database>, AllreadsError> {
        self.db.lock().map_err(|_| poisoned())
    }

    fn covers_dir(&self) -> PathBuf {
        self.data_dir.join("covers")
    }

    /// The opened book for `book_id`, opening its file on first use.
    fn opened_book(&self, book_id: i64) -> Result<Arc<OpenedBook>, AllreadsError> {
        if let Some(book) = self
            .open_books
            .lock()
            .map_err(|_| poisoned())?
            .get(&book_id)
        {
            return Ok(Arc::clone(book));
        }
        let row = self.book_row(book_id)?;
        // Parse without holding any lock; another thread may race us, which is harmless.
        let book = Arc::new(OpenedBook::open(Path::new(&row.file_path))?);
        self.open_books
            .lock()
            .map_err(|_| poisoned())?
            .insert(book_id, Arc::clone(&book));
        Ok(book)
    }

    fn book_row(&self, book_id: i64) -> Result<BookRow, AllreadsError> {
        self.db()?
            .book(book_id)?
            .ok_or_else(|| AllreadsError::Missing {
                what: format!("book {book_id}"),
            })
    }

    fn summary(db: &Database, row: BookRow) -> Result<BookSummary, AllreadsError> {
        let chapter_count = saturate_u32(db.chapter_titles(row.id)?.len());
        Ok(BookSummary {
            id: row.id,
            title: row.title,
            author: row.author,
            language_code: row.language_code,
            cover_path: row.cover_path,
            chapter_count,
            chapter_index: row.chapter_index,
            paragraph_index: row.paragraph_index,
        })
    }

    /// Deletes `path` only if it lies inside the app's data folder. Files the
    /// app did not copy in are never touched.
    fn remove_owned_file(&self, path: &Path) {
        let (Ok(file), Ok(data_dir)) = (path.canonicalize(), self.data_dir.canonicalize()) else {
            return;
        };
        if file.starts_with(&data_dir) {
            let _ = fs::remove_file(file);
        }
    }
}

// UniFFI exported methods must take owned arguments, so `String` stays by value.
#[uniffi::export]
#[allow(clippy::needless_pass_by_value)]
impl App {
    /// Opens the library in `data_dir`, creating the database on first run and
    /// adding any built-in language that is missing.
    ///
    /// # Errors
    /// Returns `DataFolderMissing` if the folder does not exist, and `Database`
    /// if the database cannot be opened or migrated.
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Result<Arc<Self>, AllreadsError> {
        let folder = PathBuf::from(&data_dir);
        if !folder.is_dir() {
            return Err(AllreadsError::DataFolderMissing { path: data_dir });
        }
        let db = Database::open(&folder.join("allreads.db"))?;
        let presets = builtin_presets().map_err(|e| AllreadsError::Database {
            message: e.to_string(),
        })?;
        for preset in &presets {
            db.add_language_if_missing(preset)?;
        }
        Ok(Arc::new(Self {
            db: Mutex::new(db),
            data_dir: folder,
            open_books: Mutex::new(HashMap::new()),
        }))
    }

    /// The version of the database schema. Useful to check after an update.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn schema_version(&self) -> Result<i64, AllreadsError> {
        Ok(self.db()?.schema_version()?)
    }

    /// The codes of the languages books can be imported in, sorted.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn language_codes(&self) -> Result<Vec<String>, AllreadsError> {
        Ok(self
            .db()?
            .languages()?
            .into_iter()
            .map(|language| language.code)
            .collect())
    }

    /// Imports the book at `path`, which Kotlin has already copied into app
    /// storage, in the given language.
    ///
    /// # Errors
    /// Returns `NotFound`, `UnsupportedFile` or `InvalidBook` for the file,
    /// `Missing` for an unknown language, and `Storage` if the cover cannot be saved.
    pub fn import_book(
        &self,
        path: String,
        language_code: String,
    ) -> Result<BookSummary, AllreadsError> {
        // The slow part (parsing) happens before the database lock is taken.
        let prepared = library::prepare_import(Path::new(&path))?;
        let db = self.db()?;
        let id = library::store_import(&db, &self.covers_dir(), &prepared, &language_code)?;
        let row = db.book(id)?.ok_or_else(|| AllreadsError::Missing {
            what: format!("book {id}"),
        })?;
        let summary = Self::summary(&db, row)?;
        drop(db);
        self.open_books
            .lock()
            .map_err(|_| poisoned())?
            .insert(id, Arc::new(prepared.book));
        Ok(summary)
    }

    /// Every book in the library, in the order they were added.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn library(&self) -> Result<Vec<BookSummary>, AllreadsError> {
        let db = self.db()?;
        db.books()?
            .into_iter()
            .map(|row| Self::summary(&db, row))
            .collect()
    }

    /// The book's chapter titles in reading order. `None` means the chapter has
    /// no heading; show "Chapter N" instead.
    ///
    /// # Errors
    /// Returns `Missing` if the book does not exist.
    pub fn chapter_titles(&self, book_id: i64) -> Result<Vec<Option<String>>, AllreadsError> {
        let db = self.db()?;
        if db.book(book_id)?.is_none() {
            return Err(AllreadsError::Missing {
                what: format!("book {book_id}"),
            });
        }
        Ok(db.chapter_titles(book_id)?)
    }

    /// One chapter as paragraphs (0-based index, reading order).
    ///
    /// # Errors
    /// Returns `Missing` for an unknown book, `NotFound` if its file is gone,
    /// and `ChapterOutOfRange` for a bad index.
    pub fn chapter_text(&self, book_id: i64, index: u32) -> Result<Vec<String>, AllreadsError> {
        let book = self.opened_book(book_id)?;
        Ok(book.chapter_text(index as usize)?)
    }

    /// Saves where the reader stopped in a book.
    ///
    /// # Errors
    /// Returns `Missing` if the book does not exist.
    pub fn save_position(
        &self,
        book_id: i64,
        chapter_index: u32,
        paragraph_index: u32,
    ) -> Result<(), AllreadsError> {
        Ok(self
            .db()?
            .save_position(book_id, chapter_index, paragraph_index)?)
    }

    /// Deletes a book, its cover, and its file if the file is in app storage.
    ///
    /// # Errors
    /// Returns `Missing` if the book does not exist.
    pub fn delete_book(&self, book_id: i64) -> Result<(), AllreadsError> {
        let row = self.book_row(book_id)?;
        self.db()?.delete_book(book_id)?;
        self.open_books
            .lock()
            .map_err(|_| poisoned())?
            .remove(&book_id);
        if let Some(cover) = &row.cover_path {
            self.remove_owned_file(Path::new(cover));
        }
        self.remove_owned_file(Path::new(&row.file_path));
        Ok(())
    }
}

/// Reads a book file's title, author, language and chapter count without
/// importing it, e.g. to suggest the language on the import screen.
///
/// # Errors
/// Returns `NotFound`, `UnsupportedFile` or `InvalidBook`.
// UniFFI exported functions must take owned arguments, so `String` stays by value.
#[uniffi::export]
#[allow(clippy::needless_pass_by_value)]
pub fn read_book_meta(path: String) -> Result<BookMeta, AllreadsError> {
    Ok(OpenedBook::open(Path::new(&path))?.meta().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use allreads_core::db::SCHEMA_VERSION;

    fn fixture(name: &str) -> String {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../core/tests/fixtures")
            .join(name)
            .display()
            .to_string()
    }

    fn new_app(dir: &tempfile::TempDir) -> Arc<App> {
        App::new(dir.path().display().to_string()).expect("app opens")
    }

    /// Copies a fixture into the app's data folder, as Kotlin's file picker would.
    fn copy_in(dir: &tempfile::TempDir, name: &str) -> String {
        let books = dir.path().join("books");
        fs::create_dir_all(&books).unwrap();
        let target = books.join(name);
        fs::copy(fixture(name), &target).unwrap();
        target.display().to_string()
    }

    #[test]
    fn app_creates_the_database_and_built_in_languages_on_first_run() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        assert_eq!(app.schema_version().unwrap(), SCHEMA_VERSION);
        assert!(dir.path().join("allreads.db").is_file());
        assert_eq!(app.language_codes().unwrap(), ["de", "en", "es", "fr"]);
    }

    #[test]
    fn app_refuses_a_missing_data_folder() {
        let result = App::new("/no/such/allreads/folder".to_string());
        assert!(matches!(
            result,
            Err(AllreadsError::DataFolderMissing { .. })
        ));
    }

    #[test]
    fn imported_books_survive_restarting_the_app() {
        let dir = tempfile::tempdir().unwrap();
        {
            let app = new_app(&dir);
            let summary = app
                .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
                .unwrap();
            assert_eq!(summary.title, "Le Petit Test");
            assert_eq!(summary.chapter_count, 2);
            app.import_book(copy_in(&dir, "sample.txt"), "fr".into())
                .unwrap();
            app.save_position(summary.id, 1, 3).unwrap();
        }
        let app = new_app(&dir);
        let library = app.library().unwrap();
        let titles: Vec<&str> = library.iter().map(|b| b.title.as_str()).collect();
        assert_eq!(titles, ["Le Petit Test", "sample"]);
        assert_eq!(
            (library[0].chapter_index, library[0].paragraph_index),
            (1, 3)
        );
        assert_eq!(library[1].chapter_count, 1);
    }

    #[test]
    fn chapters_are_read_by_book_id_after_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let id = new_app(&dir)
            .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
            .unwrap()
            .id;
        let app = new_app(&dir);
        assert_eq!(
            app.chapter_text(id, 1).unwrap(),
            ["Chapitre deux", "La suite commence ici."]
        );
        assert_eq!(
            app.chapter_titles(id).unwrap(),
            [
                Some("Chapitre un".to_string()),
                Some("Chapitre deux".to_string())
            ]
        );
        assert!(matches!(
            app.chapter_text(id, 9),
            Err(AllreadsError::ChapterOutOfRange { index: 9, count: 2 })
        ));
    }

    #[test]
    fn unknown_book_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        assert!(matches!(
            app.chapter_text(42, 0),
            Err(AllreadsError::Missing { .. })
        ));
        assert!(matches!(
            app.chapter_titles(42),
            Err(AllreadsError::Missing { .. })
        ));
        assert!(matches!(
            app.delete_book(42),
            Err(AllreadsError::Missing { .. })
        ));
    }

    #[test]
    fn importing_in_an_unknown_language_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let result = new_app(&dir).import_book(copy_in(&dir, "tiny.epub"), "zz".into());
        assert!(matches!(
            result,
            Err(AllreadsError::Missing { what }) if what == "language zz"
        ));
    }

    #[test]
    fn deleting_a_book_removes_its_cover_and_copied_file() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let path = copy_in(&dir, "layout.epub");
        let summary = app.import_book(path.clone(), "fr".into()).unwrap();
        let cover = summary.cover_path.clone().expect("layout has a cover");
        assert!(Path::new(&cover).is_file());
        app.delete_book(summary.id).unwrap();
        assert!(!Path::new(&cover).exists());
        assert!(!Path::new(&path).exists());
        assert!(app.library().unwrap().is_empty());
    }

    #[test]
    fn deleting_never_touches_a_file_outside_app_storage() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let outside = fixture("tiny.epub");
        let id = app.import_book(outside.clone(), "fr".into()).unwrap().id;
        app.delete_book(id).unwrap();
        assert!(Path::new(&outside).is_file());
    }

    #[test]
    fn typed_errors_for_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        assert!(matches!(
            app.import_book(fixture("broken.epub"), "fr".into()),
            Err(AllreadsError::InvalidBook { .. })
        ));
        assert!(matches!(
            app.import_book("/no/such/book.epub".into(), "fr".into()),
            Err(AllreadsError::NotFound { .. })
        ));
        let pdf = dir.path().join("book.pdf");
        fs::write(&pdf, b"%PDF").unwrap();
        assert!(matches!(
            app.import_book(pdf.display().to_string(), "fr".into()),
            Err(AllreadsError::UnsupportedFile { .. })
        ));
    }

    #[test]
    fn book_meta_is_read_without_importing() {
        let epub = read_book_meta(fixture("layout.epub")).unwrap();
        assert_eq!(epub.author.as_deref(), Some("Jules Auteur"));
        assert_eq!(epub.language.as_deref(), Some("fr"));
        let txt = read_book_meta(fixture("sample.txt")).unwrap();
        assert_eq!((txt.title.as_deref(), txt.chapters), (Some("sample"), 1));
    }

    #[test]
    fn core_version_is_forwarded() {
        assert_eq!(core_version(), allreads_core::version());
    }
}
