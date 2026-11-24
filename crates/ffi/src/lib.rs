//! Thin `UniFFI` wrappers over `allreads-core`. Kept deliberately small:
//! a few coarse calls that return plain records, never per-word calls.

use std::path::Path;

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
