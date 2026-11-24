//! Plain-text books: one UTF-8 file read as a single chapter.

use std::fs;
use std::path::Path;

use crate::epub::{normalize_whitespace, BookError};

/// A plain-text book. Paragraphs are separated by blank lines.
pub struct TextBook {
    title: String,
    paragraphs: Vec<String>,
}

impl TextBook {
    /// Reads the text file at `path`. The title is the file name without its extension.
    ///
    /// # Errors
    /// Returns `NotFound` if `path` is not a file, and `Invalid` if it cannot be
    /// read or is not UTF-8.
    pub fn open(path: &Path) -> Result<Self, BookError> {
        if !path.is_file() {
            return Err(BookError::NotFound(path.to_path_buf()));
        }
        let bytes = fs::read(path).map_err(|e| BookError::Invalid(e.to_string()))?;
        let text = String::from_utf8(bytes)
            .map_err(|_| BookError::Invalid("the text file is not UTF-8".into()))?;
        Ok(Self {
            title: title_from_path(path),
            paragraphs: split_paragraphs(&text),
        })
    }

    /// The book's title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// The book's paragraphs, in order.
    #[must_use]
    pub fn paragraphs(&self) -> &[String] {
        &self.paragraphs
    }
}

/// The file name without its extension, or "Untitled" if there is none.
pub(crate) fn title_from_path(path: &Path) -> String {
    path.file_stem()
        .map(|stem| stem.to_string_lossy().trim().to_string())
        .filter(|stem| !stem.is_empty())
        .unwrap_or_else(|| "Untitled".to_string())
}

/// Splits text into paragraphs at blank lines. Lines inside a paragraph are
/// joined with a space. A leading byte-order mark and Windows line endings are handled.
#[must_use]
pub fn split_paragraphs(text: &str) -> Vec<String> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut paragraphs = Vec::new();
    let mut current = String::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            push_paragraph(&mut current, &mut paragraphs);
        } else {
            current.push(' ');
            current.push_str(line);
        }
    }
    push_paragraph(&mut current, &mut paragraphs);
    paragraphs
}

fn push_paragraph(current: &mut String, paragraphs: &mut Vec<String>) {
    let paragraph = normalize_whitespace(current);
    current.clear();
    if !paragraph.is_empty() {
        paragraphs.push(paragraph);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name)
    }

    #[test]
    fn sample_file_has_two_paragraphs_and_a_title() {
        let book = TextBook::open(&fixture("sample.txt")).unwrap();
        assert_eq!(book.title(), "sample");
        assert_eq!(
            book.paragraphs(),
            [
                "Le premier paragraphe continue ici.",
                "Le deuxième paragraphe."
            ]
        );
    }

    #[test]
    fn lines_without_blank_lines_form_one_paragraph() {
        assert_eq!(split_paragraphs("un\ndeux\ntrois"), ["un deux trois"]);
    }

    #[test]
    fn whitespace_only_lines_count_as_blank() {
        assert_eq!(split_paragraphs("un\n \t \ndeux"), ["un", "deux"]);
    }

    #[test]
    fn empty_text_has_no_paragraphs() {
        assert!(split_paragraphs("").is_empty());
        assert!(split_paragraphs("\u{feff}\n\n").is_empty());
    }

    #[test]
    fn non_utf8_file_is_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("latin1.txt");
        fs::write(&path, [0x63, 0x61, 0x66, 0xe9]).unwrap();
        assert!(matches!(TextBook::open(&path), Err(BookError::Invalid(_))));
    }

    #[test]
    fn missing_file_is_not_found() {
        assert!(matches!(
            TextBook::open(Path::new("no/such.txt")),
            Err(BookError::NotFound(_))
        ));
    }

    #[test]
    fn title_falls_back_when_the_name_is_empty() {
        assert_eq!(title_from_path(Path::new(".txt")), ".txt");
        assert_eq!(title_from_path(Path::new("/")), "Untitled");
    }
}
