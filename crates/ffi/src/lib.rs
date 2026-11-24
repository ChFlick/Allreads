//! Thin `UniFFI` wrappers over `allreads-core`. Kept deliberately small:
//! a few coarse calls that return plain records, never per-word calls.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use allreads_core::db::book::Book as BookRow;
use allreads_core::db::term::{NewTerm, Status, Term};
use allreads_core::db::translation_cache::CacheKey;
use allreads_core::db::{Database, DbError};
use allreads_core::epub::BookError;
use allreads_core::library::{self, ImportError, OpenedBook};
use allreads_core::presets::builtin_presets;
use allreads_core::reader::{parse_blocks, ParsedChapter};
use allreads_core::text::{LanguageRules, PARSER_VERSION};
use allreads_core::translate::deepl::DeepL;
use allreads_core::translate::{TranslateError, TranslationRequest, Translator};

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

/// How well a word is known, for colouring it in the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WordStatus {
    /// Not saved yet.
    New,
    /// Being learned, level 1 to 5.
    Learning {
        level: u8,
    },
    Ignored,
    WellKnown,
}

impl WordStatus {
    /// The stored status, or `None` for `New` (an unsaved word).
    fn to_status(self) -> Result<Option<Status>, AllreadsError> {
        match self {
            WordStatus::New => Ok(None),
            WordStatus::Learning { level } => {
                Status::learning(level)
                    .map(Some)
                    .ok_or_else(|| AllreadsError::InvalidInput {
                        message: format!("learning level {level} is not 1 to 5"),
                    })
            }
            WordStatus::Ignored => Ok(Some(Status::Ignored)),
            WordStatus::WellKnown => Ok(Some(Status::WellKnown)),
        }
    }
}

/// A word to save from the word sheet.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TermInput {
    pub language_code: String,
    /// The word or phrase as the reader saw it.
    pub text: String,
    /// Any status but `New`; use `forget_term` to un-save a word.
    pub status: WordStatus,
    pub translation: String,
    pub notes: String,
    pub lemma: Option<String>,
}

/// A saved word, as the word sheet shows it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SavedTerm {
    pub id: i64,
    pub language_code: String,
    /// The term key, matching `Token::term_key`.
    pub term_key: String,
    /// The form as first seen.
    pub display_text: String,
    pub status: WordStatus,
    pub translation: String,
    pub notes: String,
    pub lemma: Option<String>,
}

impl From<Term> for SavedTerm {
    fn from(term: Term) -> Self {
        SavedTerm {
            id: term.id,
            language_code: term.language_code,
            term_key: term.text,
            display_text: term.display_text,
            status: term.status.into(),
            translation: term.translation,
            notes: term.notes,
            lemma: term.lemma,
        }
    }
}

impl From<Status> for WordStatus {
    fn from(status: Status) -> Self {
        match status {
            Status::Learning(level) => WordStatus::Learning { level },
            Status::Ignored => WordStatus::Ignored,
            Status::WellKnown => WordStatus::WellKnown,
        }
    }
}

/// A word, or the spaces and punctuation between words.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Token {
    pub text: String,
    /// The term key, for words only. Pass it back when saving the word.
    pub term_key: Option<String>,
    /// For words only.
    pub status: Option<WordStatus>,
    /// Sentence index within the block.
    pub sentence: u32,
    pub italic: bool,
    pub bold: bool,
}

/// A heading or paragraph.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChapterBlock {
    /// 1 to 6 for a heading, `None` for a paragraph.
    pub heading_level: Option<u8>,
    pub tokens: Vec<Token>,
}

/// A chapter ready to render.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Chapter {
    pub book_id: i64,
    pub index: u32,
    pub blocks: Vec<ChapterBlock>,
}

/// A machine translation, and whether it came from the cache (free and instant).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Translation {
    pub text: String,
    pub from_cache: bool,
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
    /// The translation service could not be reached; try again when online.
    #[error("translation is unavailable: {message}")]
    TranslationUnavailable { message: String },
    #[error("the translation service rejected the API key")]
    TranslationKeyRejected,
    #[error("the translation quota is used up")]
    TranslationQuotaExceeded,
    /// DeepL is busy even after retrying; try again in a moment.
    #[error("the translation service is busy; try again in a moment")]
    TranslationRateLimited,
    #[error("translation failed: {message}")]
    TranslationFailed { message: String },
    /// A value the caller passed in was not valid, e.g. an empty word.
    #[error("invalid input: {message}")]
    InvalidInput { message: String },
    #[error("database error: {message}")]
    Database { message: String },
}

impl From<DbError> for AllreadsError {
    fn from(error: DbError) -> Self {
        match error {
            DbError::NotFound { what } => AllreadsError::Missing { what },
            DbError::InvalidValue(message) => AllreadsError::InvalidInput { message },
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

impl From<TranslateError> for AllreadsError {
    fn from(error: TranslateError) -> Self {
        match error {
            TranslateError::Network(message) => AllreadsError::TranslationUnavailable { message },
            TranslateError::Unauthorized => AllreadsError::TranslationKeyRejected,
            TranslateError::QuotaExceeded => AllreadsError::TranslationQuotaExceeded,
            TranslateError::RateLimited => AllreadsError::TranslationRateLimited,
            other => AllreadsError::TranslationFailed {
                message: other.to_string(),
            },
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

    /// The tokenizing rules of a stored language.
    fn rules(&self, language_code: &str) -> Result<LanguageRules, AllreadsError> {
        let language =
            self.db()?
                .language(language_code)?
                .ok_or_else(|| AllreadsError::Missing {
                    what: format!("language {language_code}"),
                })?;
        LanguageRules::new(&language).map_err(|message| AllreadsError::Database { message })
    }

    /// A chapter's tokens from the cache, or parsed and cached on a miss.
    fn parsed_chapter(&self, row: &BookRow, index: u32) -> Result<ParsedChapter, AllreadsError> {
        let cached = self.db()?.cached_chapter(row.id, index, PARSER_VERSION)?;
        if let Some(chapter) = cached.as_deref().and_then(ParsedChapter::from_bytes) {
            return Ok(chapter);
        }
        // Parse without holding the database lock.
        let rules = self.rules(&row.language_code)?;
        let blocks = self.opened_book(row.id)?.chapter_blocks(index as usize)?;
        let chapter = parse_blocks(&blocks, &rules);
        // The cache only saves time: if writing it fails, the chapter still opens.
        let _ = self
            .db()?
            .store_chapter_cache(row.id, index, PARSER_VERSION, &chapter.to_bytes());
        Ok(chapter)
    }

    /// Translates through any service, using and filling the cache. The
    /// database lock is never held during the network call.
    fn translate_with(
        &self,
        translator: &dyn Translator,
        text: &str,
        context: Option<&str>,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<Translation, AllreadsError> {
        let key = CacheKey::new(text, context, source_lang, target_lang, translator.name());
        if key.text.is_empty() {
            return Err(AllreadsError::InvalidInput {
                message: "nothing to translate".into(),
            });
        }
        if let Some(cached) = self.db()?.cached_translation(&key)? {
            return Ok(Translation {
                text: cached,
                from_cache: true,
            });
        }
        let translated = translator.translate(&TranslationRequest {
            text: &key.text,
            context: key.context(),
            source: &key.source_lang,
            target: &key.target_lang,
        })?;
        self.db()?.save_translation(&key, &translated)?;
        Ok(Translation {
            text: translated,
            from_cache: false,
        })
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

    /// One chapter as blocks of tokens, each word with its term key and status.
    /// The parse is cached; statuses are always read fresh, so call this again
    /// after saving a word to recolour the chapter.
    ///
    /// # Errors
    /// Returns `Missing` for an unknown book or language, `NotFound` if the book
    /// file is gone, and `ChapterOutOfRange` for a bad index.
    pub fn open_chapter(&self, book_id: i64, index: u32) -> Result<Chapter, AllreadsError> {
        let row = self.book_row(book_id)?;
        let parsed = self.parsed_chapter(&row, index)?;
        let statuses = self.db()?.term_statuses(&row.language_code)?;
        let blocks = parsed
            .blocks
            .into_iter()
            .map(|block| ChapterBlock {
                heading_level: block.heading_level,
                tokens: block
                    .tokens
                    .into_iter()
                    .map(|token| {
                        let status = token.key.as_ref().map(|key| {
                            statuses
                                .get(key)
                                .map_or(WordStatus::New, |status| (*status).into())
                        });
                        Token {
                            text: token.text,
                            term_key: token.key,
                            status,
                            sentence: token.sentence,
                            italic: token.italic,
                            bold: token.bold,
                        }
                    })
                    .collect(),
            })
            .collect();
        Ok(Chapter {
            book_id,
            index,
            blocks,
        })
    }

    /// Saves a word (or phrase) with its status and translation. Saving a word
    /// that is already saved updates it. Call `open_chapter` again afterwards to
    /// recolour every occurrence.
    ///
    /// # Errors
    /// Returns `InvalidInput` for an empty word, the `New` status or a bad level,
    /// and `Missing` for an unknown language.
    pub fn save_term(&self, input: TermInput) -> Result<SavedTerm, AllreadsError> {
        let status = input
            .status
            .to_status()?
            .ok_or_else(|| AllreadsError::InvalidInput {
                message: "a saved word cannot be New; use forget_term".into(),
            })?;
        let db = self.db()?;
        let id = db.save_term(&NewTerm {
            language_code: input.language_code,
            text: input.text,
            lemma: input.lemma,
            status,
            translation: input.translation,
            notes: input.notes,
            parent_id: None,
        })?;
        let term = db.term(id)?.ok_or_else(|| AllreadsError::Missing {
            what: format!("term {id}"),
        })?;
        Ok(term.into())
    }

    /// The saved term for a word, for the word sheet, or `None` if it is new.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn find_term(
        &self,
        language_code: String,
        text: String,
    ) -> Result<Option<SavedTerm>, AllreadsError> {
        Ok(self
            .db()?
            .find_term(&language_code, &text)?
            .map(SavedTerm::from))
    }

    /// Un-saves a word, so it shows as new again. Returns `false` if it was not
    /// saved. Its card stays in `AnkiDroid`.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be written.
    pub fn forget_term(&self, language_code: String, text: String) -> Result<bool, AllreadsError> {
        let db = self.db()?;
        let Some(term) = db.find_term(&language_code, &text)? else {
            return Ok(false);
        };
        db.delete_term(term.id)?;
        Ok(true)
    }

    /// Marks every new word in a chapter, from block `from_block` to the end,
    /// as well known (PRD F8). Saved words keep their status. Returns how many
    /// words were added.
    ///
    /// # Errors
    /// Returns `Missing` for an unknown book, and `ChapterOutOfRange` for a bad index.
    pub fn mark_rest_known(
        &self,
        book_id: i64,
        chapter_index: u32,
        from_block: u32,
    ) -> Result<u32, AllreadsError> {
        let row = self.book_row(book_id)?;
        let parsed = self.parsed_chapter(&row, chapter_index)?;
        let words: Vec<(String, String)> = parsed
            .blocks
            .iter()
            .skip(from_block as usize)
            .flat_map(|block| &block.tokens)
            .filter_map(|token| Some((token.key.clone()?, token.text.clone())))
            .collect();
        let added = self.db()?.mark_known(&row.language_code, &words)?;
        Ok(saturate_u32(added))
    }

    /// Translates a word or sentence with DeepL. Repeats come from the cache.
    /// For a tapped word, pass its sentence as `context`: DeepL uses it to pick
    /// the right sense ("lit" as "reads" or "bed") and does not bill it.
    /// `deepl_api_key` comes from the Android Keystore for each call; it is
    /// never stored or logged.
    ///
    /// # Errors
    /// Returns `InvalidInput` for empty text or key, `TranslationUnavailable`
    /// when offline, `TranslationKeyRejected`, `TranslationQuotaExceeded`,
    /// `TranslationRateLimited`, or `TranslationFailed`.
    pub fn translate(
        &self,
        text: String,
        context: Option<String>,
        source_lang: String,
        target_lang: String,
        deepl_api_key: String,
    ) -> Result<Translation, AllreadsError> {
        if deepl_api_key.trim().is_empty() {
            return Err(AllreadsError::InvalidInput {
                message: "a DeepL API key is needed".into(),
            });
        }
        let deepl = DeepL::new(&deepl_api_key);
        self.translate_with(
            &deepl,
            &text,
            context.as_deref(),
            &source_lang,
            &target_lang,
        )
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

    fn save_word(app: &App, text: &str, status: Status) {
        app.db()
            .unwrap()
            .save_term(&allreads_core::db::term::NewTerm {
                language_code: "fr".into(),
                text: text.into(),
                lemma: None,
                status,
                translation: String::new(),
                notes: String::new(),
                parent_id: None,
            })
            .unwrap();
    }

    fn word_statuses(chapter: &Chapter) -> Vec<(String, WordStatus)> {
        chapter
            .blocks
            .iter()
            .flat_map(|block| &block.tokens)
            .filter_map(|t| Some((t.text.clone(), t.status?)))
            .collect()
    }

    #[test]
    fn open_chapter_returns_blocks_of_tokens_with_statuses() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let id = app
            .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
            .unwrap()
            .id;
        save_word(&app, "Homme", Status::Learning(2));
        let chapter = app.open_chapter(id, 0).unwrap();
        assert_eq!(chapter.blocks[0].heading_level, Some(1));
        assert_eq!(chapter.blocks[1].heading_level, None);
        let statuses = word_statuses(&chapter);
        assert!(statuses.contains(&("homme".into(), WordStatus::Learning { level: 2 })));
        assert!(statuses.contains(&("lisait".into(), WordStatus::New)));
        let italic: Vec<&str> = chapter.blocks[2]
            .tokens
            .iter()
            .filter(|t| t.italic)
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(italic, ["livre"]);
    }

    #[test]
    fn non_words_have_no_key_or_status() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let id = app
            .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
            .unwrap()
            .id;
        let chapter = app.open_chapter(id, 0).unwrap();
        for token in chapter.blocks.iter().flat_map(|b| &b.tokens) {
            assert_eq!(
                token.term_key.is_some(),
                token.status.is_some(),
                "{token:?}"
            );
            if token.term_key.is_none() {
                assert!(!token.text.chars().any(char::is_alphabetic), "{token:?}");
            }
        }
    }

    #[test]
    fn reopening_uses_the_cache_but_reads_statuses_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let id = app
            .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
            .unwrap()
            .id;
        let first = app.open_chapter(id, 0).unwrap();
        assert!(app
            .db()
            .unwrap()
            .cached_chapter(id, 0, PARSER_VERSION)
            .unwrap()
            .is_some());
        save_word(&app, "livre", Status::WellKnown);
        let second = app.open_chapter(id, 0).unwrap();
        assert!(word_statuses(&first).contains(&("livre".into(), WordStatus::New)));
        assert!(word_statuses(&second).contains(&("livre".into(), WordStatus::WellKnown)));
    }

    #[test]
    fn a_corrupt_cache_entry_is_parsed_again() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let id = app
            .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
            .unwrap()
            .id;
        let good = app.open_chapter(id, 0).unwrap();
        app.db()
            .unwrap()
            .store_chapter_cache(id, 0, PARSER_VERSION, b"garbage")
            .unwrap();
        assert_eq!(app.open_chapter(id, 0).unwrap(), good);
    }

    #[test]
    fn text_books_open_as_one_chapter_of_paragraphs() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let id = app
            .import_book(copy_in(&dir, "sample.txt"), "fr".into())
            .unwrap()
            .id;
        let chapter = app.open_chapter(id, 0).unwrap();
        assert_eq!(chapter.blocks.len(), 2);
        assert!(chapter.blocks.iter().all(|b| b.heading_level.is_none()));
        assert!(matches!(
            app.open_chapter(id, 1),
            Err(AllreadsError::ChapterOutOfRange { index: 1, count: 1 })
        ));
    }

    fn input(text: &str, status: WordStatus) -> TermInput {
        TermInput {
            language_code: "fr".into(),
            text: text.into(),
            status,
            translation: "man".into(),
            notes: String::new(),
            lemma: None,
        }
    }

    fn statuses_of(chapter: &Chapter, key: &str) -> Vec<WordStatus> {
        chapter
            .blocks
            .iter()
            .flat_map(|b| &b.tokens)
            .filter(|t| t.term_key.as_deref() == Some(key))
            .filter_map(|t| t.status)
            .collect()
    }

    /// Step 13's check on the Rust side: saving a word recolours every occurrence.
    #[test]
    fn saving_a_word_recolours_every_occurrence_in_the_chapter() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let id = app
            .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
            .unwrap()
            .id;
        let before = app.open_chapter(id, 0).unwrap();
        assert_eq!(
            statuses_of(&before, "homme"),
            [WordStatus::New, WordStatus::New]
        );

        let saved = app
            .save_term(input("homme", WordStatus::Learning { level: 2 }))
            .unwrap();
        assert_eq!(saved.term_key, "homme");

        let after = app.open_chapter(id, 0).unwrap();
        let learning = WordStatus::Learning { level: 2 };
        assert_eq!(statuses_of(&after, "homme"), [learning, learning]);
    }

    #[test]
    fn find_term_returns_the_saved_translation_for_any_casing() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        app.save_term(input("Homme", WordStatus::Learning { level: 1 }))
            .unwrap();
        let found = app.find_term("fr".into(), "HOMME".into()).unwrap().unwrap();
        assert_eq!(
            (found.display_text.as_str(), found.translation.as_str()),
            ("Homme", "man")
        );
        assert_eq!(app.find_term("fr".into(), "femme".into()).unwrap(), None);
    }

    #[test]
    fn forgetting_a_word_makes_it_new_again() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        app.save_term(input("homme", WordStatus::Ignored)).unwrap();
        assert!(app.forget_term("fr".into(), "homme".into()).unwrap());
        assert!(!app.forget_term("fr".into(), "homme".into()).unwrap());
        assert_eq!(app.find_term("fr".into(), "homme".into()).unwrap(), None);
    }

    #[test]
    fn saving_with_new_or_a_bad_level_is_invalid_input() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        for status in [WordStatus::New, WordStatus::Learning { level: 7 }] {
            assert!(matches!(
                app.save_term(input("homme", status)),
                Err(AllreadsError::InvalidInput { .. })
            ));
        }
        assert!(matches!(
            app.save_term(input("   ", WordStatus::WellKnown)),
            Err(AllreadsError::InvalidInput { .. })
        ));
    }

    #[test]
    fn mark_rest_known_skips_earlier_blocks_and_saved_words() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let id = app
            .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
            .unwrap()
            .id;
        app.save_term(input("homme", WordStatus::Learning { level: 3 }))
            .unwrap();
        // Block 0 is the heading "Chapitre un"; start at block 1.
        let added = app.mark_rest_known(id, 0, 1).unwrap();
        assert!(added > 0);
        let chapter = app.open_chapter(id, 0).unwrap();
        assert_eq!(statuses_of(&chapter, "chapitre"), [WordStatus::New]);
        assert_eq!(statuses_of(&chapter, "lisait"), [WordStatus::WellKnown]);
        assert_eq!(
            statuses_of(&chapter, "homme"),
            [
                WordStatus::Learning { level: 3 },
                WordStatus::Learning { level: 3 }
            ]
        );
        assert_eq!(app.mark_rest_known(id, 0, 1).unwrap(), 0);
    }

    fn mock_deepl(status: u16, body: &str) -> (DeepL, std::thread::JoinHandle<String>) {
        // A tiny local server stands in for DeepL; it answers exactly one request.
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let body = body.to_string();
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buffer = [0; 4096];
            let read = stream.read(&mut buffer).unwrap();
            let response = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\n\
                 Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            stream.write_all(response.as_bytes()).unwrap();
            String::from_utf8_lossy(&buffer[..read]).to_string()
        });
        (DeepL::with_base_url("test-key:fx", &url), handle)
    }

    /// Step 14's check on the Rust side: the second lookup is instant (cached).
    #[test]
    fn a_second_translation_comes_from_the_cache_without_a_request() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let (deepl, server) = mock_deepl(200, r#"{"translations":[{"text":"The man reads."}]}"#);
        let first = app
            .translate_with(&deepl, "L'homme  lit.", None, "fr", "en")
            .unwrap();
        assert_eq!(
            first,
            Translation {
                text: "The man reads.".into(),
                from_cache: false
            }
        );
        server.join().unwrap();
        // The mock server is gone: this only works if no request is made.
        let second = app
            .translate_with(&deepl, "L'homme lit.", None, "fr", "en")
            .unwrap();
        assert_eq!(
            second,
            Translation {
                text: "The man reads.".into(),
                from_cache: true
            }
        );
    }

    #[test]
    fn a_word_is_cached_per_sentence_it_was_translated_in() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let (deepl, server) = mock_deepl(200, r#"{"translations":[{"text":"reads"}]}"#);
        app.translate_with(&deepl, "lit", Some("Il lit."), "fr", "en")
            .unwrap();
        server.join().unwrap();
        let (deepl, server) = mock_deepl(200, r#"{"translations":[{"text":"bed"}]}"#);
        let bed = app
            .translate_with(&deepl, "lit", Some("Le lit."), "fr", "en")
            .unwrap();
        server.join().unwrap();
        assert_eq!(
            bed,
            Translation {
                text: "bed".into(),
                from_cache: false
            }
        );
        let offline = DeepL::with_base_url("k", "http://127.0.0.1:9");
        let reads = app
            .translate_with(&offline, "lit", Some("Il lit."), "fr", "en")
            .unwrap();
        assert_eq!(
            reads,
            Translation {
                text: "reads".into(),
                from_cache: true
            }
        );
    }

    #[test]
    fn translation_errors_are_typed_and_not_cached() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let (deepl, server) = mock_deepl(456, "{}");
        assert!(matches!(
            app.translate_with(&deepl, "chat", None, "fr", "en"),
            Err(AllreadsError::TranslationQuotaExceeded)
        ));
        server.join().unwrap();
        let (deepl, server) = mock_deepl(403, "{}");
        assert!(matches!(
            app.translate_with(&deepl, "chat", None, "fr", "en"),
            Err(AllreadsError::TranslationKeyRejected)
        ));
        server.join().unwrap();
    }

    #[test]
    fn being_offline_is_translation_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let offline = DeepL::with_base_url("k", "http://127.0.0.1:9");
        assert!(matches!(
            app.translate_with(&offline, "chat", None, "fr", "en"),
            Err(AllreadsError::TranslationUnavailable { .. })
        ));
    }

    #[test]
    fn empty_text_or_key_is_invalid_input() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        assert!(matches!(
            app.translate("chat".into(), None, "fr".into(), "en".into(), "  ".into()),
            Err(AllreadsError::InvalidInput { .. })
        ));
        let offline = DeepL::with_base_url("k", "http://127.0.0.1:9");
        assert!(matches!(
            app.translate_with(&offline, "   ", None, "fr", "en"),
            Err(AllreadsError::InvalidInput { .. })
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
            app.open_chapter(42, 0),
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
