//! Installing a downloaded catalog, as for dictionaries: verify the
//! checksum, unpack, check it opens, move it into place. A file that fails
//! any check is deleted, so a broken catalog is never opened.

use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use serde::Deserialize;

use super::{Catalog, CatalogError};
use crate::dictionary::file_sha256;

/// A catalog offered for download, from the `catalogs` list of the
/// dictionaries manifest.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AvailableCatalog {
    /// The books' language, e.g. `fr`.
    pub language: String,
    pub url: String,
    /// Build date, e.g. `2026-12-01`.
    pub built: String,
    pub books: u32,
    pub download_bytes: u64,
    pub installed_bytes: u64,
    /// SHA-256 of the downloaded `.gz`, in hex.
    pub sha256: String,
}

/// An installed catalog, for Settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledCatalog {
    pub language: String,
    pub built: String,
    pub books: u32,
    pub bytes: u64,
}

#[derive(Deserialize)]
struct Manifest {
    format: u32,
    #[serde(default)]
    catalogs: Vec<AvailableCatalog>,
}

/// The catalogs listed in the dictionaries manifest that this app can use.
/// A manifest without catalogs lists none.
///
/// # Errors
/// Returns `Format` for JSON that is not a manifest.
pub fn parse_catalog_manifest(json: &str) -> Result<Vec<AvailableCatalog>, CatalogError> {
    let manifest: Manifest =
        serde_json::from_str(json).map_err(|e| CatalogError::Format(format!("manifest: {e}")))?;
    if manifest.format != crate::dictionary::FORMAT_VERSION {
        return Ok(Vec::new());
    }
    Ok(manifest
        .catalogs
        .into_iter()
        .filter(|c| valid_language(&c.language) && c.url.starts_with("https://"))
        .collect())
}

/// A language code like `fr`: two or three lower-case letters. It becomes a
/// file name, so nothing else is allowed.
#[must_use]
pub fn valid_language(code: &str) -> bool {
    (2..=3).contains(&code.len()) && code.bytes().all(|b| b.is_ascii_lowercase())
}

fn file_for(dir: &Path, language: &str) -> PathBuf {
    dir.join(format!("{language}.sqlite"))
}

/// The installed catalog for `language` in `dir`, if there is one.
#[must_use]
pub fn installed_path(dir: &Path, language: &str) -> Option<PathBuf> {
    let path = file_for(dir, language);
    (valid_language(language) && path.is_file()).then_some(path)
}

/// Installs the gzipped catalog at `download` for `language` in `dir`,
/// replacing an earlier one. `sha256` is the manifest's checksum. The
/// download itself is left for the caller to delete.
///
/// # Errors
/// Returns `ChecksumMismatch` for a damaged download, `Format` for a file
/// that is not a catalog for `language`, and `Io` if writing fails.
pub fn install(
    download: &Path,
    sha256: &str,
    language: &str,
    dir: &Path,
) -> Result<InstalledCatalog, CatalogError> {
    if !valid_language(language) {
        return Err(CatalogError::Format(format!(
            "bad language code {language:?}"
        )));
    }
    let actual =
        file_sha256(download).map_err(|e| CatalogError::Io(io::Error::other(e.to_string())))?;
    if !actual.eq_ignore_ascii_case(sha256.trim()) {
        return Err(CatalogError::ChecksumMismatch);
    }
    fs::create_dir_all(dir)?;
    let partial = dir.join(format!("{language}.sqlite.partial"));
    if let Err(error) = unpack_and_check(download, &partial, language) {
        let _ = fs::remove_file(&partial);
        return Err(error);
    }
    // Rename is atomic on one file system: the old file is replaced in one step.
    let target = file_for(dir, language);
    fs::rename(&partial, &target)?;
    describe(&target)
}

fn unpack_and_check(download: &Path, partial: &Path, language: &str) -> Result<(), CatalogError> {
    {
        let mut input = GzDecoder::new(File::open(download)?);
        let mut output = File::create(partial)?;
        io::copy(&mut input, &mut output)
            .map_err(|e| CatalogError::Format(format!("unpacking: {e}")))?;
        output.flush()?;
        output.sync_all()?;
    }
    let catalog = Catalog::open(partial).map_err(|e| match e {
        CatalogError::Sql(e) => CatalogError::Format(e.to_string()),
        other => other,
    })?;
    if catalog.language_code() != language {
        return Err(CatalogError::Format(format!(
            "the file is for {}, not {language}",
            catalog.language_code()
        )));
    }
    Ok(())
}

fn describe(path: &Path) -> Result<InstalledCatalog, CatalogError> {
    let catalog = Catalog::open(path)?;
    Ok(InstalledCatalog {
        language: catalog.language_code().to_string(),
        built: catalog.built().to_string(),
        books: u32::try_from(catalog.books().len()).unwrap_or(u32::MAX),
        bytes: fs::metadata(path)?.len(),
    })
}

/// The installed catalogs in `dir`, sorted by language. Unreadable files are skipped.
///
/// # Errors
/// Returns `Io` if `dir` exists but cannot be read.
pub fn installed(dir: &Path) -> Result<Vec<InstalledCatalog>, CatalogError> {
    let mut found = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(found),
        Err(e) => return Err(e.into()),
    };
    for entry in entries {
        let path = entry?.path();
        let usable = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".sqlite"))
            .is_some_and(valid_language);
        if let (true, Ok(info)) = (usable, describe(&path)) {
            found.push(info);
        }
    }
    found.sort_by(|a, b| a.language.cmp(&b.language));
    Ok(found)
}

/// Deletes an installed catalog. Returns `false` if it was not installed.
///
/// # Errors
/// Returns `Format` for a bad language code, and `Io` if deleting fails.
pub fn remove(dir: &Path, language: &str) -> Result<bool, CatalogError> {
    if !valid_language(language) {
        return Err(CatalogError::Format(format!(
            "bad language code {language:?}"
        )));
    }
    match fs::remove_file(file_for(dir, language)) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use flate2::write::GzEncoder;
    use flate2::Compression;

    use super::*;
    use crate::catalog::{CatalogWriter, NewCatalogBook};
    use crate::dictionary::file_sha256;

    /// A gzipped one-book catalog for `language`, and its checksum.
    fn download(dir: &Path, language: &str) -> (PathBuf, String) {
        let plain = dir.join(format!("{language}-plain.sqlite"));
        let mut writer = CatalogWriter::create(&plain, language, "x", "2026-12-01").unwrap();
        let book = NewCatalogBook {
            id: 1,
            title: "Un".into(),
            author: "A".into(),
            subjects: Vec::new(),
            url: "https://example.org/1.epub".into(),
            popularity: 1,
            year: None,
        };
        writer.add_book(&book, &HashMap::new()).unwrap();
        writer.finish().unwrap();
        let gz = dir.join(format!("{language}.sqlite.gz"));
        let mut encoder = GzEncoder::new(File::create(&gz).unwrap(), Compression::default());
        encoder.write_all(&fs::read(&plain).unwrap()).unwrap();
        encoder.finish().unwrap();
        let sha = file_sha256(&gz).unwrap();
        (gz, sha)
    }

    #[test]
    fn a_catalog_is_installed_listed_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let installed_dir = dir.path().join("catalogs");
        let (gz, sha) = download(dir.path(), "fr");
        let info = install(&gz, &sha, "fr", &installed_dir).unwrap();
        assert_eq!(info.language, "fr");
        assert_eq!(info.built, "2026-12-01");
        assert_eq!(info.books, 1);
        assert_eq!(installed(&installed_dir).unwrap(), [info]);
        assert!(installed_path(&installed_dir, "fr").is_some());
        assert!(remove(&installed_dir, "fr").unwrap());
        assert!(!remove(&installed_dir, "fr").unwrap());
        assert!(installed(&installed_dir).unwrap().is_empty());
    }

    #[test]
    fn a_damaged_or_wrong_download_is_refused_and_leaves_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let installed_dir = dir.path().join("catalogs");
        let (gz, sha) = download(dir.path(), "de");
        assert!(matches!(
            install(&gz, "00", "de", &installed_dir),
            Err(CatalogError::ChecksumMismatch)
        ));
        // A German catalog offered as French.
        assert!(matches!(
            install(&gz, &sha, "fr", &installed_dir),
            Err(CatalogError::Format(_))
        ));
        assert!(matches!(
            install(&gz, &sha, "../x", &installed_dir),
            Err(CatalogError::Format(_))
        ));
        assert!(installed(&installed_dir).unwrap().is_empty());
        assert_eq!(fs::read_dir(&installed_dir).unwrap().count(), 0);
    }

    #[test]
    fn the_manifest_lists_usable_catalogs_and_none_when_it_has_none() {
        let json = r#"{"format": 1, "dictionaries": [], "catalogs": [
            {"language": "fr", "url": "https://example.org/fr.sqlite.gz", "built": "2026-12-01",
             "books": 480, "download_bytes": 6000000, "installed_bytes": 12000000, "sha256": "ab"},
            {"language": "../x", "url": "https://example.org/x", "built": "x",
             "books": 1, "download_bytes": 1, "installed_bytes": 1, "sha256": "00"},
            {"language": "de", "url": "http://example.org/de.sqlite.gz", "built": "x",
             "books": 1, "download_bytes": 1, "installed_bytes": 1, "sha256": "00"}
        ]}"#;
        let catalogs = parse_catalog_manifest(json).unwrap();
        assert_eq!(catalogs.len(), 1);
        assert_eq!(catalogs[0].language, "fr");
        assert_eq!(catalogs[0].books, 480);
        assert!(
            parse_catalog_manifest(r#"{"format": 1, "dictionaries": []}"#)
                .unwrap()
                .is_empty()
        );
        assert!(parse_catalog_manifest(r#"{"format": 99, "catalogs": []}"#)
            .unwrap()
            .is_empty());
        assert!(parse_catalog_manifest("nope").is_err());
    }
}
