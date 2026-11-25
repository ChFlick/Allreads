//! Reading EPUB files: metadata, cover, chapter titles and chapter blocks.

use std::path::Path;

use scraper::{Html, Selector};

use crate::book::{BookError, BookMeta};
use crate::text::blocks::{html_to_blocks, Block};
use crate::text::normalize_whitespace;

/// An open EPUB. Opening parses the archive once; reads then reuse it.
pub struct Book {
    epub: rbook::Epub,
}

impl Book {
    /// Opens the EPUB at `path`.
    ///
    /// # Errors
    /// Returns `NotFound` if `path` is not a file, and `Invalid` if the file is not a readable EPUB.
    pub fn open(path: &Path) -> Result<Self, BookError> {
        if !path.is_file() {
            return Err(BookError::NotFound(path.to_path_buf()));
        }
        let epub = rbook::Epub::open(path).map_err(|e| BookError::Invalid(e.to_string()))?;
        Ok(Self { epub })
    }

    /// The book's title, author, language and chapter count.
    #[must_use]
    pub fn meta(&self) -> BookMeta {
        let metadata = self.epub.metadata();
        let creators: Vec<_> = metadata.creators().collect();
        // Prefer a creator whose main role is "aut", then one with no role,
        // and finally whoever is listed first.
        let author = creators
            .iter()
            .find(|c| c.main_role().is_some_and(|role| role.code() == "aut"))
            .or_else(|| creators.iter().find(|c| c.main_role().is_none()))
            .or_else(|| creators.first())
            .map(|c| c.value().to_string());

        BookMeta {
            title: metadata.title().map(|t| t.value().to_string()),
            author,
            language: metadata.language().map(|l| l.value().to_string()),
            chapters: self.epub.reader().len(),
        }
    }

    /// Number of readable chapters in reading order.
    #[must_use]
    pub fn chapter_count(&self) -> usize {
        self.epub.reader().len()
    }

    /// The cover image and a file extension for it, if the book has a cover in
    /// a common image format.
    #[must_use]
    pub fn cover(&self) -> Option<(Vec<u8>, &'static str)> {
        let entry = self.epub.manifest().cover_image()?;
        let extension = match entry.media_type() {
            "image/jpeg" => "jpg",
            "image/png" => "png",
            "image/gif" => "gif",
            "image/webp" => "webp",
            _ => return None,
        };
        let bytes = entry.read_bytes().ok()?;
        Some((bytes, extension))
    }

    /// Each chapter's title, taken from its first heading. `None` for a chapter
    /// without a heading.
    ///
    /// # Errors
    /// Returns `Invalid` if a chapter cannot be read.
    pub fn chapter_titles(&self) -> Result<Vec<Option<String>>, BookError> {
        let reader = self.epub.reader();
        (0..reader.len())
            .map(|index| {
                let content = reader
                    .get(index)
                    .map_err(|e| BookError::Invalid(e.to_string()))?;
                Ok(first_heading(content.content()))
            })
            .collect()
    }

    /// One chapter's blocks with their inline styles (0-based, in reading order).
    ///
    /// # Errors
    /// Returns `ChapterOutOfRange` if `index` is past the last chapter, and `Invalid` if the chapter cannot be read.
    pub fn chapter_blocks(&self, index: usize) -> Result<Vec<Block>, BookError> {
        let reader = self.epub.reader();
        let count = reader.len();
        if index >= count {
            return Err(BookError::ChapterOutOfRange { index, count });
        }
        let content = reader
            .get(index)
            .map_err(|e| BookError::Invalid(e.to_string()))?;
        Ok(html_to_blocks(content.content()))
    }
}

/// The text of the first heading (`h1` to `h6`), if there is a non-blank one.
fn first_heading(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let headings = Selector::parse("h1, h2, h3, h4, h5, h6").expect("selector is valid");
    document
        .select(&headings)
        .map(|heading| normalize_whitespace(&heading.text().collect::<String>()))
        .find(|text| !text.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// A chapter's blocks as plain strings.
    fn paragraphs(book: &Book, index: usize) -> Result<Vec<String>, BookError> {
        Ok(book
            .chapter_blocks(index)?
            .iter()
            .map(Block::text)
            .collect())
    }

    /// XHTML as plain strings, one per block.
    fn html_to_paragraphs(html: &str) -> Vec<String> {
        html_to_blocks(html).iter().map(Block::text).collect()
    }

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn reads_metadata_from_epub() {
        let meta = Book::open(&fixture("tiny.epub")).unwrap().meta();
        assert_eq!(meta.title.as_deref(), Some("Le Petit Test"));
        assert_eq!(meta.author.as_deref(), Some("Jean Exemple"));
        assert_eq!(meta.language.as_deref(), Some("fr"));
        assert_eq!(meta.chapters, 2);
    }

    #[test]
    fn reads_first_chapter_as_paragraphs() {
        let book = Book::open(&fixture("tiny.epub")).unwrap();
        assert_eq!(
            paragraphs(&book, 0).unwrap(),
            vec![
                "Chapitre un".to_string(),
                "Il était une fois un homme qui lisait.".to_string(),
                "Le livre était très long.".to_string(),
                "L’homme s’arrêta.".to_string(),
            ]
        );
    }

    #[test]
    fn one_book_serves_several_chapters() {
        let book = Book::open(&fixture("tiny.epub")).unwrap();
        assert_eq!(book.chapter_count(), 2);
        assert_eq!(
            paragraphs(&book, 1).unwrap(),
            vec!["Chapitre deux", "La suite commence ici."]
        );
        assert_eq!(paragraphs(&book, 0).unwrap().len(), 4);
    }

    #[test]
    fn missing_file_is_not_found() {
        let missing = Path::new("does/not/exist.epub");
        assert_eq!(
            Book::open(missing).err(),
            Some(BookError::NotFound(missing.to_path_buf()))
        );
    }

    #[test]
    fn non_epub_file_is_invalid() {
        let result = Book::open(&fixture("broken.epub"));
        assert!(matches!(result, Err(BookError::Invalid(_))));
    }

    #[test]
    fn chapter_past_the_end_is_out_of_range() {
        let book = Book::open(&fixture("tiny.epub")).unwrap();
        assert_eq!(
            paragraphs(&book, 5),
            Err(BookError::ChapterOutOfRange { index: 5, count: 2 })
        );
    }

    #[test]
    fn author_prefers_the_author_role_over_an_illustrator() {
        let meta = Book::open(&fixture("layout.epub")).unwrap().meta();
        assert_eq!(meta.author.as_deref(), Some("Jules Auteur"));
    }

    #[test]
    fn layout_chapter_keeps_text_from_divs_and_bare_body() {
        let book = Book::open(&fixture("layout.epub")).unwrap();
        assert_eq!(
            paragraphs(&book, 0).unwrap(),
            vec![
                "Texte nu.",
                "Premier paragraphe & co.",
                "Un deux",
                "Il dit\u{a0}: oui.",
                "Citation",
                "Un",
                "Deux trois",
            ]
        );
    }

    #[test]
    fn chapter_titles_come_from_the_first_heading() {
        let tiny = Book::open(&fixture("tiny.epub")).unwrap();
        assert_eq!(
            tiny.chapter_titles().unwrap(),
            vec![
                Some("Chapitre un".to_string()),
                Some("Chapitre deux".to_string())
            ]
        );
        let layout = Book::open(&fixture("layout.epub")).unwrap();
        assert_eq!(layout.chapter_titles().unwrap(), vec![None, None]);
    }

    #[test]
    fn blank_headings_are_skipped_for_the_title() {
        assert_eq!(
            first_heading("<h1> </h1><h2>Deux</h2>"),
            Some("Deux".to_string())
        );
        assert_eq!(first_heading("<p>Pas de titre</p>"), None);
    }

    #[test]
    fn cover_is_read_when_the_book_has_one() {
        let layout = Book::open(&fixture("layout.epub")).unwrap();
        let (bytes, extension) = layout.cover().expect("layout fixture has a cover");
        assert_eq!(extension, "png");
        assert!(bytes.starts_with(b"\x89PNG"));
        assert!(Book::open(&fixture("tiny.epub")).unwrap().cover().is_none());
    }

    #[test]
    fn cover_is_not_a_chapter() {
        let layout = Book::open(&fixture("layout.epub")).unwrap();
        assert_eq!(layout.chapter_count(), 2);
    }

    #[test]
    fn headings_keep_their_text_and_drop_blank_blocks() {
        assert_eq!(
            html_to_paragraphs("<h2>Titre</h2><h3>   </h3><p> </p>"),
            vec!["Titre".to_string()]
        );
    }

    #[test]
    fn inline_markup_stays_in_its_block() {
        assert_eq!(
            html_to_paragraphs("<p>Un <i>deux</i>, <b>trois</b>.</p>"),
            vec!["Un deux, trois.".to_string()]
        );
    }

    #[test]
    fn nested_blocks_are_not_duplicated() {
        assert_eq!(
            html_to_paragraphs("<div><div><p>Once</p></div></div>"),
            vec!["Once".to_string()]
        );
    }

    #[test]
    fn scripts_and_styles_are_ignored() {
        assert_eq!(
            html_to_paragraphs("<p>Avant</p><script>alert(1)</script><style>p{}</style>"),
            vec!["Avant".to_string()]
        );
    }

    #[test]
    fn non_breaking_space_is_kept() {
        assert_eq!(
            html_to_paragraphs("<p>Il dit\u{a0}: non.</p>"),
            vec!["Il dit\u{a0}: non.".to_string()]
        );
    }

    #[test]
    fn line_breaks_become_spaces() {
        assert_eq!(
            html_to_paragraphs("<p>ligne un<br/>ligne deux</p>"),
            vec!["ligne un ligne deux".to_string()]
        );
    }

    #[test]
    fn entities_are_decoded() {
        assert_eq!(
            html_to_paragraphs("<p>Tom &amp; Jerry &lt;3</p>"),
            vec!["Tom & Jerry <3".to_string()]
        );
    }
}
