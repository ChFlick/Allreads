//! Thin UniFFI wrappers over `allreads-core`. Kept deliberately small:
//! a few coarse calls that return plain records, never per-word calls.

use std::path::Path;

uniffi::setup_scaffolding!();

/// Returns the core version.
#[uniffi::export]
pub fn core_version() -> String {
    allreads_core::version().to_string()
}

/// Book facts for the library screen. Mirrors `allreads_core::epub::BookMeta`.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BookMeta {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub chapters: u32,
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
                index: index as u32,
                count: count as u32,
            },
        }
    }
}

/// Reads an EPUB's title, author, language and chapter count.
#[uniffi::export]
pub fn read_epub_meta(path: String) -> Result<BookMeta, AllreadsError> {
    let meta = allreads_core::epub::read_meta(Path::new(&path))?;
    Ok(BookMeta {
        title: meta.title,
        author: meta.author,
        language: meta.language,
        chapters: meta.chapters as u32,
    })
}

/// Returns one chapter as paragraphs (0-based index, reading order).
#[uniffi::export]
pub fn chapter_text(path: String, index: u32) -> Result<Vec<String>, AllreadsError> {
    Ok(allreads_core::epub::chapter_text(
        Path::new(&path),
        index as usize,
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> String {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../core/tests/fixtures/tiny.epub")
            .display()
            .to_string()
    }

    #[test]
    fn core_version_is_forwarded() {
        assert_eq!(core_version(), allreads_core::version());
    }

    #[test]
    fn meta_crosses_the_boundary() {
        let meta = read_epub_meta(fixture()).expect("fixture should open");
        assert_eq!(meta.title.as_deref(), Some("Le Petit Test"));
        assert_eq!(meta.chapters, 2);
    }

    #[test]
    fn invalid_path_becomes_not_found_error() {
        let result = read_epub_meta("/no/such/book.epub".to_string());
        assert!(matches!(result, Err(AllreadsError::NotFound { .. })));
    }

    #[test]
    fn chapter_out_of_range_becomes_typed_error() {
        let result = chapter_text(fixture(), 9);
        assert!(matches!(
            result,
            Err(AllreadsError::ChapterOutOfRange { index: 9, count: 2 })
        ));
    }
}
