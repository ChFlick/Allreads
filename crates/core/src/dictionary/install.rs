//! Installing a downloaded dictionary: verify, unpack, check, move into place.
//! A file that fails any check is deleted, so a broken dictionary is never opened.

use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};

use super::{Dictionary, DictionaryError};

/// An installed dictionary, for Settings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledDictionary {
    pub pair: String,
    /// Build date from the file, e.g. `2026-11-25`.
    pub built: String,
    pub bytes: u64,
}

/// A pair like `fr-en`: two or three lower-case letters on each side. It
/// becomes a file name, so nothing else is allowed.
#[must_use]
pub fn valid_pair(pair: &str) -> bool {
    let side = |s: &str| (2..=3).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_lowercase());
    matches!(pair.split_once('-'), Some((a, b)) if side(a) && side(b))
}

fn file_for(dir: &Path, pair: &str) -> PathBuf {
    dir.join(format!("{pair}.sqlite"))
}

/// Installs the gzipped dictionary at `download` as `pair` in `dir`,
/// replacing an earlier one. `sha256` is the manifest's checksum of the
/// download. The download itself is left for the caller to delete.
///
/// # Errors
/// Returns `ChecksumMismatch` for a damaged download, `Invalid` for a file
/// that is not a dictionary for `pair`, and `Io` if writing fails.
pub fn install(
    download: &Path,
    sha256: &str,
    pair: &str,
    dir: &Path,
) -> Result<InstalledDictionary, DictionaryError> {
    if !valid_pair(pair) {
        return Err(DictionaryError::Invalid(format!("bad pair name {pair:?}")));
    }
    if !file_sha256(download)?.eq_ignore_ascii_case(sha256.trim()) {
        return Err(DictionaryError::ChecksumMismatch);
    }
    fs::create_dir_all(dir)?;
    let partial = dir.join(format!("{pair}.sqlite.partial"));
    let result = unpack_and_check(download, &partial, pair);
    if let Err(error) = result {
        let _ = fs::remove_file(&partial);
        return Err(error);
    }
    // Rename is atomic on one file system: the old file is replaced in one step.
    let target = file_for(dir, pair);
    fs::rename(&partial, &target)?;
    describe(&target, pair)
}

fn unpack_and_check(download: &Path, partial: &Path, pair: &str) -> Result<(), DictionaryError> {
    {
        let mut input = GzDecoder::new(File::open(download)?);
        let mut output = File::create(partial)?;
        io::copy(&mut input, &mut output)
            .map_err(|e| DictionaryError::Invalid(format!("unpacking: {e}")))?;
        output.flush()?;
        output.sync_all()?;
    }
    let dictionary = Dictionary::open(partial).map_err(|e| match e {
        DictionaryError::Sqlite(e) => DictionaryError::Invalid(e.to_string()),
        other => other,
    })?;
    let stored = dictionary.meta("pair")?;
    if stored.as_deref() != Some(pair) {
        return Err(DictionaryError::Invalid(format!(
            "the file is for {}, not {pair}",
            stored.unwrap_or_else(|| "nothing".into())
        )));
    }
    dictionary.check()
}

fn describe(path: &Path, pair: &str) -> Result<InstalledDictionary, DictionaryError> {
    let built = Dictionary::open(path)?.meta("built")?.unwrap_or_default();
    Ok(InstalledDictionary {
        pair: pair.to_string(),
        built,
        bytes: fs::metadata(path)?.len(),
    })
}

/// The installed dictionaries in `dir`, sorted by pair. Unreadable files are skipped.
///
/// # Errors
/// Returns `Io` if `dir` exists but cannot be read.
pub fn installed(dir: &Path) -> Result<Vec<InstalledDictionary>, DictionaryError> {
    let mut found = Vec::new();
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(found),
        Err(e) => return Err(e.into()),
    };
    for entry in entries {
        let path = entry?.path();
        let Some(pair) = path
            .file_name()
            .and_then(|n| n.to_str())
            .and_then(|n| n.strip_suffix(".sqlite"))
            .filter(|p| valid_pair(p))
        else {
            continue;
        };
        if let Ok(info) = describe(&path, pair) {
            found.push(info);
        }
    }
    found.sort_by(|a, b| a.pair.cmp(&b.pair));
    Ok(found)
}

/// Deletes an installed dictionary. Returns `false` if it was not installed.
///
/// # Errors
/// Returns `Invalid` for a bad pair name, and `Io` if deleting fails.
pub fn remove(dir: &Path, pair: &str) -> Result<bool, DictionaryError> {
    if !valid_pair(pair) {
        return Err(DictionaryError::Invalid(format!("bad pair name {pair:?}")));
    }
    match fs::remove_file(file_for(dir, pair)) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// The SHA-256 of a file, in lower-case hex, as the manifest lists it.
///
/// # Errors
/// Returns `Io` if the file cannot be read.
pub fn file_sha256(path: &Path) -> Result<String, DictionaryError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        // Writing to a String cannot fail.
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(hex)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::test_support::write_tiny;
    use crate::dictionary::FORMAT_VERSION;
    use flate2::write::GzEncoder;
    use flate2::Compression;

    /// A gzipped tiny dictionary and its checksum, as the release would have.
    fn download(dir: &Path, pair: &str) -> (PathBuf, String) {
        let plain = dir.join("plain.sqlite");
        write_tiny(&plain, pair, FORMAT_VERSION);
        let gz = dir.join("download.gz");
        let mut encoder = GzEncoder::new(File::create(&gz).unwrap(), Compression::fast());
        encoder.write_all(&fs::read(&plain).unwrap()).unwrap();
        encoder.finish().unwrap();
        let sum = file_sha256(&gz).unwrap();
        (gz, sum)
    }

    #[test]
    fn a_good_download_installs_and_can_be_looked_up_and_removed() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("dictionaries");
        let (gz, sum) = download(temp.path(), "fr-en");
        let info = install(&gz, &sum.to_uppercase(), "fr-en", &dir).unwrap();
        assert_eq!(info.built, "2026-11-25");
        assert!(info.bytes > 0);
        assert_eq!(installed(&dir).unwrap(), [info]);
        let dictionary = Dictionary::open(&dir.join("fr-en.sqlite")).unwrap();
        assert_eq!(
            dictionary.lookup("lisait").unwrap().suggested_lemma(),
            Some("lire")
        );
        assert!(!dir.join("fr-en.sqlite.partial").exists());
        assert!(remove(&dir, "fr-en").unwrap());
        assert!(!remove(&dir, "fr-en").unwrap());
        assert!(installed(&dir).unwrap().is_empty());
    }

    #[test]
    fn a_damaged_download_is_refused_and_leaves_nothing() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("dictionaries");
        let (gz, _) = download(temp.path(), "fr-en");
        assert!(matches!(
            install(&gz, "00", "fr-en", &dir),
            Err(DictionaryError::ChecksumMismatch)
        ));
        assert!(installed(&dir).unwrap().is_empty());
    }

    #[test]
    fn a_truncated_download_with_a_matching_checksum_is_refused() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("dictionaries");
        let (gz, _) = download(temp.path(), "fr-en");
        let bytes = fs::read(&gz).unwrap();
        fs::write(&gz, &bytes[..bytes.len() / 2]).unwrap();
        let sum = file_sha256(&gz).unwrap();
        assert!(matches!(
            install(&gz, &sum, "fr-en", &dir),
            Err(DictionaryError::Invalid(_))
        ));
        assert!(
            fs::read_dir(&dir).unwrap().next().is_none(),
            "the partial file is deleted"
        );
    }

    #[test]
    fn a_file_for_another_pair_is_refused() {
        let temp = tempfile::tempdir().unwrap();
        let (gz, sum) = download(temp.path(), "de-en");
        let result = install(&gz, &sum, "fr-en", &temp.path().join("d"));
        assert!(matches!(result, Err(DictionaryError::Invalid(_))));
    }

    #[test]
    fn installing_again_replaces_the_old_file() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path().join("dictionaries");
        let (gz, sum) = download(temp.path(), "fr-en");
        install(&gz, &sum, "fr-en", &dir).unwrap();
        install(&gz, &sum, "fr-en", &dir).unwrap();
        assert_eq!(installed(&dir).unwrap().len(), 1);
    }

    #[test]
    fn pair_names_cannot_escape_the_folder() {
        assert!(valid_pair("fr-en"));
        assert!(valid_pair("fra-eng"));
        for bad in ["../fr-en", "fr", "FR-en", "fr-en-x", "f-en", "fr-e/"] {
            assert!(!valid_pair(bad), "{bad}");
        }
    }
}
