//! Thin `UniFFI` wrappers over `allreads-core`. Kept deliberately small:
//! a few coarse calls that return plain records, never per-word calls.
//!
//! - `records`: the data types Kotlin sees.
//! - `error`: `AllreadsError`, the exception type Kotlin sees.
//! - `app`: `App`, the object with every call.

mod app;
mod error;
mod records;

pub use app::{check_deepl_key, read_book_meta, App};
pub use error::AllreadsError;
pub use records::{
    AnkiCardTemplate, AnkiNote, AnkiNoteType, BookMeta, BookSummary, Chapter, ChapterBlock,
    DeepLKeyStatus, LanguageInfo, SavedTerm, TermInput, Token, Translation, WordStatus,
};

uniffi::setup_scaffolding!();

/// Returns the core version.
#[uniffi::export]
#[must_use]
pub fn core_version() -> String {
    allreads_core::version().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_version_is_forwarded() {
        assert_eq!(core_version(), allreads_core::version());
    }
}
