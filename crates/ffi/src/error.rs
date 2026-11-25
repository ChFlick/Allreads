//! `AllreadsError`: every error Kotlin can see, and how core errors map to it.

use allreads_core::book::BookError;
use allreads_core::db::DbError;
use allreads_core::library::ImportError;
use allreads_core::translate::TranslateError;

use crate::records::saturate_u32;

/// Errors that cross into Kotlin as typed exceptions.
///
/// No field may be called `message`: it would clash with Kotlin's
/// `Exception.message`, and the generated bindings would not compile.
#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum AllreadsError {
    #[error("file not found: {path}")]
    NotFound { path: String },
    #[error("could not read the book: {reason}")]
    InvalidBook { reason: String },
    #[error("unsupported file type: {reason}")]
    UnsupportedFile { reason: String },
    #[error("chapter {index} is out of range; the book has {count} chapters")]
    ChapterOutOfRange { index: u32, count: u32 },
    #[error("data folder not found: {path}")]
    DataFolderMissing { path: String },
    /// A book, language or other row that does not exist.
    #[error("{what} not found")]
    Missing { what: String },
    #[error("could not write a file: {reason}")]
    Storage { reason: String },
    /// The translation service could not be reached; try again when online.
    #[error("translation is unavailable: {reason}")]
    TranslationUnavailable { reason: String },
    #[error("the translation service rejected the API key")]
    TranslationKeyRejected,
    #[error("the translation quota is used up")]
    TranslationQuotaExceeded,
    /// DeepL is busy even after retrying; try again in a moment.
    #[error("the translation service is busy; try again in a moment")]
    TranslationRateLimited,
    #[error("translation failed: {reason}")]
    TranslationFailed { reason: String },
    /// A value the caller passed in was not valid, e.g. an empty word.
    #[error("invalid input: {reason}")]
    InvalidInput { reason: String },
    #[error("database error: {reason}")]
    Database { reason: String },
}

impl From<DbError> for AllreadsError {
    fn from(error: DbError) -> Self {
        match error {
            DbError::NotFound { what } => AllreadsError::Missing { what },
            DbError::InvalidValue(reason) => AllreadsError::InvalidInput { reason },
            other => AllreadsError::Database {
                reason: other.to_string(),
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
            BookError::Invalid(reason) => AllreadsError::InvalidBook { reason },
            BookError::Unsupported(reason) => AllreadsError::UnsupportedFile { reason },
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
            ImportError::Cover(reason) => AllreadsError::Storage { reason },
        }
    }
}

impl From<TranslateError> for AllreadsError {
    fn from(error: TranslateError) -> Self {
        match error {
            TranslateError::Network(reason) => AllreadsError::TranslationUnavailable { reason },
            TranslateError::Unauthorized => AllreadsError::TranslationKeyRejected,
            TranslateError::QuotaExceeded => AllreadsError::TranslationQuotaExceeded,
            TranslateError::RateLimited => AllreadsError::TranslationRateLimited,
            other => AllreadsError::TranslationFailed {
                reason: other.to_string(),
            },
        }
    }
}

/// The error for a lock left poisoned by an earlier panic.
pub(crate) fn poisoned() -> AllreadsError {
    AllreadsError::Database {
        reason: "a lock is poisoned after an earlier crash".into(),
    }
}
