//! Thin `UniFFI` wrappers over `allreads-core`. Kept deliberately small:
//! a few coarse calls that return plain records, never per-word calls.

use std::path::Path;
use std::sync::{Arc, Mutex};

use allreads_core::db::Database;

uniffi::setup_scaffolding!();

/// Returns the core version.
#[uniffi::export]
#[must_use]
pub fn core_version() -> String {
    allreads_core::version().to_string()
}

/// Book facts for the library screen.
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

/// Converts a count to `u32`, clamping instead of wrapping.
fn saturate_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// Errors that cross into Kotlin as typed exceptions.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum AllreadsError {
    #[error("file not found: {path}")]
    NotFound { path: String },
    #[error("could not read EPUB: {message}")]
    InvalidEpub { message: String },
    #[error("chapter {index} is out of range; the book has {count} chapters")]
    ChapterOutOfRange { index: u32, count: u32 },
    #[error("data folder not found: {path}")]
    DataFolderMissing { path: String },
    #[error("database error: {message}")]
    Database { message: String },
}

impl From<allreads_core::db::DbError> for AllreadsError {
    fn from(error: allreads_core::db::DbError) -> Self {
        AllreadsError::Database {
            message: error.to_string(),
        }
    }
}

impl From<allreads_core::epub::EpubError> for AllreadsError {
    fn from(error: allreads_core::epub::EpubError) -> Self {
        use allreads_core::epub::EpubError;
        match error {
            EpubError::NotFound(path) => AllreadsError::NotFound {
                path: path.display().to_string(),
            },
            EpubError::Invalid(message) => AllreadsError::InvalidEpub { message },
            EpubError::ChapterOutOfRange { index, count } => AllreadsError::ChapterOutOfRange {
                index: saturate_u32(index),
                count: saturate_u32(count),
            },
        }
    }
}

/// The app's library: one `SQLite` database in the app's data folder.
///
/// Create it once and keep it for the life of the app. Kotlin calls it on
/// `Dispatchers.IO`, so the database sits behind a `Mutex`.
#[derive(uniffi::Object)]
pub struct App {
    db: Mutex<Database>,
}

#[uniffi::export]
impl App {
    /// Opens the library in `data_dir`, creating the database on first run.
    ///
    /// # Errors
    /// Returns `DataFolderMissing` if the folder does not exist, and `Database`
    /// if the database cannot be opened or migrated.
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Result<Arc<Self>, AllreadsError> {
        let folder = Path::new(&data_dir);
        if !folder.is_dir() {
            return Err(AllreadsError::DataFolderMissing { path: data_dir });
        }
        let db = Database::open(&folder.join("allreads.db"))?;
        Ok(Arc::new(Self { db: Mutex::new(db) }))
    }

    /// The version of the database schema. Useful to check after an update.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn schema_version(&self) -> Result<i64, AllreadsError> {
        let db = self.db.lock().map_err(|_| AllreadsError::Database {
            message: "database lock is poisoned".into(),
        })?;
        Ok(db.schema_version()?)
    }
}

/// Reads an EPUB's title, author, language and chapter count.
///
/// # Errors
/// Returns `NotFound` for a missing file and `InvalidEpub` for a file that is not a readable EPUB.
// UniFFI exported functions must take owned arguments, so `String` stays by value.
#[uniffi::export]
#[allow(clippy::needless_pass_by_value)]
pub fn read_epub_meta(path: String) -> Result<BookMeta, AllreadsError> {
    let book = allreads_core::epub::Book::open(Path::new(&path))?;
    Ok(book.meta().into())
}

/// Returns one chapter as paragraphs (0-based index, reading order).
///
/// # Errors
/// Returns `NotFound`, `InvalidEpub` or `ChapterOutOfRange`.
// UniFFI exported functions must take owned arguments, so `String` stays by value.
#[uniffi::export]
#[allow(clippy::needless_pass_by_value)]
pub fn chapter_text(path: String, index: u32) -> Result<Vec<String>, AllreadsError> {
    let book = allreads_core::epub::Book::open(Path::new(&path))?;
    Ok(book.chapter_text(index as usize)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> String {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../core/tests/fixtures")
            .join(name)
            .display()
            .to_string()
    }

    #[test]
    fn app_creates_the_database_on_first_run() {
        let dir = tempfile::tempdir().unwrap();
        let app = App::new(dir.path().display().to_string()).expect("app opens");
        assert_eq!(app.schema_version().unwrap(), 1);
        assert!(dir.path().join("allreads.db").is_file());
    }

    #[test]
    fn app_reopens_an_existing_library() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().display().to_string();
        drop(App::new(folder.clone()).unwrap());
        let reopened = App::new(folder).expect("second open succeeds");
        assert_eq!(reopened.schema_version().unwrap(), 1);
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
    fn core_version_is_forwarded() {
        assert_eq!(core_version(), allreads_core::version());
    }

    #[test]
    fn meta_crosses_the_boundary() {
        let meta = read_epub_meta(fixture("tiny.epub")).expect("fixture should open");
        assert_eq!(meta.title.as_deref(), Some("Le Petit Test"));
        assert_eq!(meta.chapters, 2);
    }

    #[test]
    fn broken_file_becomes_invalid_epub_error() {
        let result = read_epub_meta(fixture("broken.epub"));
        assert!(matches!(result, Err(AllreadsError::InvalidEpub { .. })));
    }

    #[test]
    fn author_role_is_respected_across_the_boundary() {
        let meta = read_epub_meta(fixture("layout.epub")).expect("layout fixture opens");
        assert_eq!(meta.author.as_deref(), Some("Jules Auteur"));
    }

    #[test]
    fn invalid_path_becomes_not_found_error() {
        let result = read_epub_meta("/no/such/book.epub".to_string());
        assert!(matches!(result, Err(AllreadsError::NotFound { .. })));
    }

    #[test]
    fn chapter_out_of_range_becomes_typed_error() {
        let result = chapter_text(fixture("tiny.epub"), 9);
        assert!(matches!(
            result,
            Err(AllreadsError::ChapterOutOfRange { index: 9, count: 2 })
        ));
    }
}
