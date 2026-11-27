//! "Find a book": the calls for downloaded book catalogs.

use std::path::Path;

use allreads_core::catalog::{self, Catalog};

use crate::app::App;
use crate::error::{poisoned, AllreadsError};
use crate::records::{AvailableCatalog, CatalogBook, CatalogWord, InstalledCatalog};

impl App {
    /// Runs `f` on the language's installed catalog, opening it on first use.
    fn with_catalog<T>(
        &self,
        language_code: &str,
        f: impl FnOnce(&Catalog) -> Result<T, AllreadsError>,
    ) -> Result<T, AllreadsError> {
        let mut open = self.catalogs.lock().map_err(|_| poisoned())?;
        if !open.contains_key(language_code) {
            let path =
                catalog::installed_path(&self.catalogs_dir(), language_code).ok_or_else(|| {
                    AllreadsError::Missing {
                        what: format!("book catalog for {language_code}"),
                    }
                })?;
            open.insert(language_code.to_string(), Catalog::open(&path)?);
        }
        f(&open[language_code])
    }
}

#[uniffi::export]
impl App {
    /// Every book in the language's catalog with how well it fits the saved
    /// words, most popular first. One call per screen: the fit reads every
    /// book's word counts.
    ///
    /// # Errors
    /// Returns `Missing` if no catalog is installed for the language, and
    /// `CatalogInvalid` for a damaged one.
    pub fn catalog_books(&self, language_code: String) -> Result<Vec<CatalogBook>, AllreadsError> {
        // Read the statuses first, so the database is not locked while the
        // catalog is.
        let statuses = self.db()?.term_statuses(&language_code)?;
        self.with_catalog(&language_code, |catalog| {
            let fits = catalog.fit(&statuses)?;
            Ok(catalog
                .books()
                .iter()
                .zip(fits)
                .map(|(book, fit)| CatalogBook {
                    id: book.id,
                    title: book.title.clone(),
                    author: book.author.clone(),
                    subjects: book.subjects.clone(),
                    url: book.url.clone(),
                    year: book.year,
                    popularity: book.popularity,
                    difficulty: fit.difficulty.into(),
                    worth_learning: fit.worth_learning,
                })
                .collect())
        })
    }

    /// The ids of the books whose title, author or subjects contain every word
    /// of `query`, ignoring case and accents. Cheap: call it as the reader types.
    ///
    /// # Errors
    /// Returns `Missing` if no catalog is installed for the language.
    pub fn search_catalog(
        &self,
        language_code: String,
        query: String,
    ) -> Result<Vec<i64>, AllreadsError> {
        self.with_catalog(&language_code, |catalog| {
            Ok(catalog
                .search(&query)
                .into_iter()
                .map(|book| book.id)
                .collect())
        })
    }

    /// The new words a catalog book uses most, at most `limit`: what reading it
    /// would teach.
    ///
    /// # Errors
    /// Returns `Missing` if no catalog is installed for the language, and
    /// `CatalogInvalid` for an unknown book.
    pub fn catalog_new_words(
        &self,
        language_code: String,
        book_id: i64,
        limit: u32,
    ) -> Result<Vec<CatalogWord>, AllreadsError> {
        let statuses = self.db()?.term_statuses(&language_code)?;
        self.with_catalog(&language_code, |catalog| {
            let limit = usize::try_from(limit).unwrap_or(usize::MAX);
            Ok(catalog
                .top_new_words(book_id, &statuses, limit)?
                .into_iter()
                .map(|(word, count)| CatalogWord { word, count })
                .collect())
        })
    }

    /// The installed book catalogs, for Settings.
    ///
    /// # Errors
    /// Returns `Storage` if the catalogs folder cannot be read.
    pub fn installed_catalogs(&self) -> Result<Vec<InstalledCatalog>, AllreadsError> {
        Ok(catalog::installed(&self.catalogs_dir())?
            .into_iter()
            .map(InstalledCatalog::from)
            .collect())
    }

    /// Installs a downloaded catalog (the `.gz` at `download_path`) after
    /// checking it against the manifest's `sha256`. Kotlin deletes the
    /// download afterwards, whatever the result.
    ///
    /// # Errors
    /// Returns `DictionaryDamaged` for a corrupt download, `CatalogInvalid`
    /// for a file that is not a catalog for the language, and `Storage` if it
    /// cannot be written.
    pub fn install_catalog(
        &self,
        download_path: String,
        sha256: String,
        language_code: String,
    ) -> Result<InstalledCatalog, AllreadsError> {
        // Close an open copy, and hold the lock until the new file is in
        // place, so no other call opens the old file in between.
        let mut open = self.catalogs.lock().map_err(|_| poisoned())?;
        open.remove(&language_code);
        let info = catalog::install(
            Path::new(&download_path),
            &sha256,
            &language_code,
            &self.catalogs_dir(),
        )?;
        drop(open);
        Ok(info.into())
    }

    /// Deletes an installed catalog. Returns `false` if it was not installed.
    ///
    /// # Errors
    /// Returns `CatalogInvalid` for a bad language code, `Storage` if deleting fails.
    pub fn remove_catalog(&self, language_code: String) -> Result<bool, AllreadsError> {
        // Held until the file is gone, so no other call reopens it in between.
        let mut open = self.catalogs.lock().map_err(|_| poisoned())?;
        open.remove(&language_code);
        let removed = catalog::remove(&self.catalogs_dir(), &language_code)?;
        drop(open);
        Ok(removed)
    }
}

/// The book catalogs listed in the dictionaries manifest (`dictionaries.json`).
///
/// # Errors
/// Returns `CatalogInvalid` for JSON that is not a manifest.
#[uniffi::export]
pub fn parse_catalog_manifest(json: String) -> Result<Vec<AvailableCatalog>, AllreadsError> {
    Ok(catalog::parse_catalog_manifest(&json)?
        .into_iter()
        .map(AvailableCatalog::from)
        .collect())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs::File;
    use std::io::Write as _;
    use std::sync::Arc;

    use allreads_core::catalog::{CatalogWriter, NewCatalogBook};
    use allreads_core::dictionary::file_sha256;
    use allreads_core::word_tally::BookWord;
    use flate2::write::GzEncoder;
    use flate2::Compression;

    use super::*;
    use crate::records::{TermInput, WordStatus};

    /// A gzipped French catalog with one book ("le" ×6, "chat" ×4), and its checksum.
    fn download(dir: &Path) -> (String, String) {
        let plain = dir.join("plain.sqlite");
        let mut writer = CatalogWriter::create(&plain, "fr", "x", "2026-12-01").unwrap();
        let word = |count| BookWord {
            count,
            lemma_key: None,
            is_name: false,
        };
        let words: HashMap<String, BookWord> =
            [("le".to_string(), word(6)), ("chat".to_string(), word(4))]
                .into_iter()
                .collect();
        let book = NewCatalogBook {
            id: 7,
            title: "Les Chats".into(),
            author: "A. Auteur".into(),
            subjects: vec!["Contes".into()],
            url: "https://example.org/7.epub".into(),
            popularity: 3,
            year: Some(1910),
        };
        writer.add_book(&book, &words).unwrap();
        writer.finish().unwrap();
        let gz = dir.join("fr.sqlite.gz");
        let mut encoder = GzEncoder::new(File::create(&gz).unwrap(), Compression::default());
        encoder.write_all(&std::fs::read(&plain).unwrap()).unwrap();
        encoder.finish().unwrap();
        let sha = file_sha256(&gz).unwrap();
        (gz.display().to_string(), sha)
    }

    fn new_app(dir: &tempfile::TempDir) -> Arc<App> {
        App::new(dir.path().display().to_string()).expect("app opens")
    }

    #[test]
    fn an_installed_catalog_shows_each_book_against_the_saved_words() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        assert!(matches!(
            app.catalog_books("fr".into()),
            Err(AllreadsError::Missing { .. })
        ));
        let (gz, sha) = download(dir.path());
        let info = app.install_catalog(gz, sha, "fr".into()).unwrap();
        assert_eq!((info.books, info.built.as_str()), (1, "2026-12-01"));
        assert_eq!(app.installed_catalogs().unwrap(), [info]);

        let before = app.catalog_books("fr".into()).unwrap();
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].year, Some(1910));
        assert!((before[0].difficulty.new_percent - 100.0).abs() < 1e-9);

        app.save_term(TermInput {
            language_code: "fr".into(),
            text: "le".into(),
            status: WordStatus::WellKnown,
            translation: String::new(),
            notes: String::new(),
            lemma: None,
            sentence: None,
            sentence_translation: None,
            source_book: None,
            source_chapter: None,
        })
        .unwrap();
        let after = app.catalog_books("fr".into()).unwrap();
        assert!((after[0].difficulty.new_percent - 40.0).abs() < 1e-9);
        assert_eq!(
            app.catalog_new_words("fr".into(), 7, 10).unwrap(),
            [CatalogWord {
                word: "chat".into(),
                count: 4
            }]
        );
        assert_eq!(
            app.search_catalog("fr".into(), "contes".into()).unwrap(),
            [7]
        );
        assert!(app
            .search_catalog("fr".into(), "zola".into())
            .unwrap()
            .is_empty());

        assert!(app.remove_catalog("fr".into()).unwrap());
        assert!(matches!(
            app.catalog_books("fr".into()),
            Err(AllreadsError::Missing { .. })
        ));
    }

    #[test]
    fn a_damaged_catalog_download_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let (gz, _) = download(dir.path());
        assert!(matches!(
            app.install_catalog(gz, "00".into(), "fr".into()),
            Err(AllreadsError::DictionaryDamaged)
        ));
        assert!(app.installed_catalogs().unwrap().is_empty());
    }
}
