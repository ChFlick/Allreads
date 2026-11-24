//! Reading EPUB files: book metadata and chapter text as plain paragraphs.

use std::path::{Path, PathBuf};

use rbook::prelude::*;
use scraper::{Html, Selector};
use thiserror::Error;

/// Basic facts about a book, read from its EPUB metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct BookMeta {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    /// Number of readable chapters in reading order.
    pub chapters: usize,
}

/// Errors from reading an EPUB file.
#[derive(Debug, Error, PartialEq)]
pub enum EpubError {
    #[error("file not found: {}", .0.display())]
    NotFound(PathBuf),
    #[error("could not read EPUB: {0}")]
    Invalid(String),
    #[error("chapter {index} is out of range; the book has {count} chapters")]
    ChapterOutOfRange { index: usize, count: usize },
}

fn open(path: &Path) -> Result<rbook::Epub, EpubError> {
    if !path.exists() {
        return Err(EpubError::NotFound(path.to_path_buf()));
    }
    rbook::Epub::open(path).map_err(|e| EpubError::Invalid(e.to_string()))
}

/// Reads the title, first author, language and chapter count of an EPUB file.
pub fn read_meta(path: &Path) -> Result<BookMeta, EpubError> {
    let epub = open(path)?;
    let metadata = epub.metadata();
    let title = metadata.title().map(|t| t.value().to_string());
    let author = metadata.creators().next().map(|c| c.value().to_string());
    let language = metadata.language().map(|l| l.value().to_string());
    let chapters = epub.reader().len();
    Ok(BookMeta {
        title,
        author,
        language,
        chapters,
    })
}

/// Returns the paragraphs of one chapter (0-based, in reading order).
pub fn chapter_text(path: &Path, index: usize) -> Result<Vec<String>, EpubError> {
    let epub = open(path)?;
    let reader = epub.reader();
    let count = reader.len();
    if index >= count {
        return Err(EpubError::ChapterOutOfRange { index, count });
    }
    let content = reader
        .get(index)
        .map_err(|e| EpubError::Invalid(e.to_string()))?;
    Ok(html_to_paragraphs(content.content()))
}

/// Turns XHTML into one string per heading or paragraph.
///
/// Whitespace is collapsed to single spaces, and empty blocks are dropped.
/// Inline markup such as `<i>` and `<b>` is flattened into plain text for now.
pub fn html_to_paragraphs(html: &str) -> Vec<String> {
    let document = Html::parse_document(html);
    let blocks = Selector::parse("h1, h2, h3, h4, h5, h6, p").expect("selector is valid");
    document
        .select(&blocks)
        .map(|block| block.text().collect::<Vec<_>>().join(""))
        .map(|text| text.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|text| !text.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny.epub")
    }

    #[test]
    fn reads_metadata_from_epub() {
        let meta = read_meta(&fixture()).expect("fixture should open");
        assert_eq!(meta.title.as_deref(), Some("Le Petit Test"));
        assert_eq!(meta.author.as_deref(), Some("Jean Exemple"));
        assert_eq!(meta.language.as_deref(), Some("fr"));
        assert_eq!(meta.chapters, 2);
    }

    #[test]
    fn reads_first_chapter_as_paragraphs() {
        let paragraphs = chapter_text(&fixture(), 0).expect("chapter 0 should load");
        assert_eq!(
            paragraphs,
            vec![
                "Chapitre un".to_string(),
                "Il était une fois un homme qui lisait.".to_string(),
                "Le livre était très long.".to_string(),
                "L’homme s’arrêta.".to_string(),
            ]
        );
    }

    #[test]
    fn missing_file_is_not_found() {
        let missing = Path::new("does/not/exist.epub");
        assert_eq!(
            read_meta(missing),
            Err(EpubError::NotFound(missing.to_path_buf()))
        );
    }

    #[test]
    fn chapter_past_the_end_is_out_of_range() {
        assert_eq!(
            chapter_text(&fixture(), 5),
            Err(EpubError::ChapterOutOfRange { index: 5, count: 2 })
        );
    }

    #[test]
    fn html_keeps_headings_and_drops_empty_blocks() {
        let html = "<h2>Titre</h2><p>  Un   <i>deux</i> </p><p> </p>";
        assert_eq!(
            html_to_paragraphs(html),
            vec!["Titre".to_string(), "Un deux".to_string()]
        );
    }
}
