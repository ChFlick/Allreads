//! Building a book catalog for "Find a book": counts each book's words with
//! the app's own rules and writes them with [`CatalogWriter`].
//!
//! The input is a folder of EPUB files named `<id>.epub` and a list with one
//! JSON object per line describing them:
//!
//! ```text
//! {"id": 4650, "title": "Candide", "author": "Voltaire", "subjects": ["Satire"],
//!  "url": "https://www.gutenberg.org/ebooks/4650.epub3.images", "popularity": 1200}
//! ```
//!
//! Fetching the books and writing the list is the workflow's job, so this
//! part works the same for any source.

use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use allreads_core::catalog::{CatalogWriter, NewCatalogBook};
use allreads_core::dictionary::{Dictionary, DictionaryError, WordKinds};
use allreads_core::library::OpenedBook;
use allreads_core::presets::builtin_presets;
use allreads_core::text::LanguageRules;
use allreads_core::word_tally::{count_book, count_stamp, dictionary_identity, BookWord, Lookup};
use serde::Deserialize;

/// One line of the book list.
#[derive(Debug, Deserialize)]
struct ListedBook {
    id: i64,
    title: String,
    #[serde(default)]
    author: String,
    #[serde(default)]
    subjects: Vec<String>,
    url: String,
    #[serde(default)]
    popularity: u32,
}

/// What a build did, for the log.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CatalogSummary {
    pub books: usize,
    /// Listed books whose EPUB was missing or could not be read.
    pub skipped: Vec<String>,
    pub running_words: u64,
}

/// Builds a catalog at `output` from the books in `list` and `books_dir`,
/// counted for `language` with the offline dictionary at `dictionary` (as the
/// app counts them once that dictionary is installed).
///
/// # Errors
/// Returns a message if the list, the dictionary or the output cannot be
/// used. A single unreadable book is skipped and reported instead.
pub fn build_catalog(
    list: &Path,
    books_dir: &Path,
    dictionary: Option<&Path>,
    language: &str,
    output: &Path,
    min_count: u32,
) -> Result<CatalogSummary, String> {
    let preset = builtin_presets()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|p| p.code == language)
        .ok_or_else(|| format!("no preset for language {language}"))?;
    let rules = LanguageRules::new(&preset).map_err(|e| e.to_string())?;
    let dictionary = dictionary
        .map(|path| Dictionary::open(path).map_err(|e| format!("{}: {e}", path.display())))
        .transpose()?;
    let identity = match &dictionary {
        Some(d) => Some(dictionary_identity(
            language,
            d.meta("built").map_err(|e| e.to_string())?.as_deref(),
        )),
        None => None,
    };
    let stamp = count_stamp(identity.as_deref());
    let lookup = |key: &str| -> Result<WordKinds, DictionaryError> {
        match &dictionary {
            Some(d) => d.word_kinds(key),
            None => Ok(WordKinds::default()),
        }
    };
    let lookup_ref: &dyn Fn(&str) -> Result<WordKinds, DictionaryError> = &lookup;

    let mut writer =
        CatalogWriter::create(output, language, &stamp, min_count).map_err(|e| e.to_string())?;
    let mut summary = CatalogSummary::default();
    let lines = BufReader::new(File::open(list).map_err(|e| format!("{}: {e}", list.display()))?);
    for (number, line) in lines.lines().enumerate() {
        let line = line.map_err(|e| e.to_string())?;
        if line.trim().is_empty() {
            continue;
        }
        let listed: ListedBook = serde_json::from_str(&line)
            .map_err(|e| format!("{} line {}: {e}", list.display(), number + 1))?;
        let path = books_dir.join(format!("{}.epub", listed.id));
        let words = match count(
            &path,
            &rules,
            language,
            dictionary.as_ref().map(|_| lookup_ref),
        ) {
            Ok(words) => words,
            Err(reason) => {
                summary
                    .skipped
                    .push(format!("{} ({}): {reason}", listed.id, listed.title));
                continue;
            }
        };
        let book = NewCatalogBook {
            id: listed.id,
            title: listed.title,
            author: listed.author,
            subjects: listed.subjects,
            url: listed.url,
            popularity: listed.popularity,
        };
        writer.add_book(&book, &words).map_err(|e| e.to_string())?;
        summary.books += 1;
        summary.running_words += words
            .values()
            .filter(|w| !w.is_name)
            .map(|w| u64::from(w.count))
            .sum::<u64>();
    }
    writer.finish().map_err(|e| e.to_string())?;
    Ok(summary)
}

/// One book's words, counted as the app counts a library book.
fn count(
    path: &Path,
    rules: &LanguageRules,
    language: &str,
    lookup: Lookup,
) -> Result<HashMap<String, BookWord>, String> {
    let book = OpenedBook::open(path).map_err(|e| e.to_string())?;
    let chapters = book.chapter_titles().map_err(|e| e.to_string())?.len();
    count_book(&book, chapters, rules, language, lookup).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use allreads_core::catalog::Catalog;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../core/tests/fixtures")
            .join(name)
    }

    #[test]
    fn listed_books_are_counted_into_a_catalog_and_missing_ones_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let books = dir.path().join("books");
        std::fs::create_dir(&books).unwrap();
        std::fs::copy(fixture("tiny.epub"), books.join("1.epub")).unwrap();
        std::fs::copy(fixture("layout.epub"), books.join("2.epub")).unwrap();
        let list = dir.path().join("books.jsonl");
        std::fs::write(
            &list,
            concat!(
                r#"{"id": 1, "title": "Tiny", "author": "A", "subjects": ["Tales"], "url": "https://example.org/1.epub", "popularity": 5}"#,
                "\n",
                r#"{"id": 2, "title": "Layout", "url": "https://example.org/2.epub", "popularity": 9}"#,
                "\n\n",
                r#"{"id": 3, "title": "Missing", "url": "https://example.org/3.epub"}"#,
                "\n",
            ),
        )
        .unwrap();
        let output = dir.path().join("fr.sqlite");
        let summary = build_catalog(&list, &books, None, "fr", &output, 1).unwrap();
        assert_eq!(summary.books, 2);
        assert_eq!(summary.skipped.len(), 1);
        assert!(summary.skipped[0].starts_with("3 (Missing)"));

        let catalog = Catalog::open(&output).unwrap();
        assert_eq!(catalog.language_code(), "fr");
        // No dictionary: the same stamp as the app without one.
        assert_eq!(catalog.count_stamp(), count_stamp(None));
        let ids: Vec<i64> = catalog.books().iter().map(|b| b.id).collect();
        assert_eq!(ids, [2, 1]);
        let total: u64 = catalog.books().iter().map(|b| b.words).sum();
        assert_eq!(total, summary.running_words);
        assert!(total > 0);
        // Nothing saved: every word is new.
        for fit in catalog.fit(&HashMap::new()).unwrap() {
            assert_eq!(fit.difficulty.new_words, fit.difficulty.words);
        }
    }

    #[test]
    fn a_bad_list_line_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let list = dir.path().join("books.jsonl");
        std::fs::write(&list, "{not json}\n").unwrap();
        let error = build_catalog(
            &list,
            dir.path(),
            None,
            "fr",
            &dir.path().join("out.sqlite"),
            1,
        )
        .unwrap_err();
        assert!(error.contains("line 1"), "{error}");
    }
}
