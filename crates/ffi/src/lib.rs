//! Thin `UniFFI` wrappers over `allreads-core`. Kept deliberately small:
//! a few coarse calls that return plain records, never per-word calls.
//!
//! - `records`: the data types Kotlin sees.
//! - `error`: `AllreadsError`, the exception type Kotlin sees.
//! - `app`: `App`, the object with every call.
//! - `catalog`: `App`'s calls for downloaded book catalogs ("Find a book").
//! - `vocab_check`: the vocabulary check session and its calls.

mod app;
mod catalog;
mod error;
mod records;
mod vocab_check;

pub use app::{check_deepl_key, parse_dictionary_manifest, read_book_meta, App};
pub use catalog::parse_catalog_manifest;
pub use error::AllreadsError;
pub use records::{
    AnkiCardTemplate, AnkiNote, AnkiNoteType, AvailableDictionary, BookMeta, BookSummary, Chapter,
    ChapterBlock, DeepLKeyStatus, DictionaryEntry, DictionaryForm, DictionaryLookup,
    InstalledDictionary, LanguageInfo, PhraseMark, PhraseSuggestion, SavedTerm, TermInput, Token,
    Translation, WordStatus,
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
