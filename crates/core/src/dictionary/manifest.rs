//! The `dictionaries.json` manifest published next to the files.

use serde::Deserialize;

use super::DictionaryError;

/// A dictionary offered for download.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct AvailableDictionary {
    /// e.g. `fr-en`.
    pub pair: String,
    /// The book language it is for, e.g. `fr`.
    pub language: String,
    pub url: String,
    /// Build date, e.g. `2026-11-25`.
    pub built: String,
    pub download_bytes: u64,
    pub installed_bytes: u64,
    /// SHA-256 of the downloaded `.gz`, in hex.
    pub sha256: String,
}

#[derive(Deserialize)]
struct Manifest {
    format: u32,
    dictionaries: Vec<AvailableDictionary>,
}

/// Reads the manifest, keeping only dictionaries this app can use.
///
/// # Errors
/// Returns `Invalid` for JSON that is not a manifest.
pub fn parse_manifest(json: &str) -> Result<Vec<AvailableDictionary>, DictionaryError> {
    let manifest: Manifest = serde_json::from_str(json)
        .map_err(|e| DictionaryError::Invalid(format!("manifest: {e}")))?;
    if manifest.format != super::FORMAT_VERSION {
        // A newer app is needed; offer nothing rather than a file we cannot read.
        return Ok(Vec::new());
    }
    Ok(manifest
        .dictionaries
        .into_iter()
        .filter(|d| super::valid_pair(&d.pair) && d.url.starts_with("https://"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANIFEST: &str = r#"{
      "format": 1,
      "dictionaries": [
        {"pair": "fr-en", "language": "fr", "url": "https://example.org/fr-en.sqlite.gz",
         "built": "2026-11-25", "download_bytes": 10773550, "installed_bytes": 42651648,
         "sha256": "ab12"},
        {"pair": "../evil", "language": "fr", "url": "https://example.org/x",
         "built": "x", "download_bytes": 1, "installed_bytes": 1, "sha256": "00"}
      ]
    }"#;

    #[test]
    fn the_manifest_lists_usable_dictionaries() {
        let list = parse_manifest(MANIFEST).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].pair, "fr-en");
        assert_eq!(list[0].download_bytes, 10_773_550);
    }

    #[test]
    fn a_newer_format_offers_nothing() {
        let json = MANIFEST.replace("\"format\": 1", "\"format\": 2");
        assert!(parse_manifest(&json).unwrap().is_empty());
    }

    #[test]
    fn garbage_is_an_error() {
        assert!(matches!(
            parse_manifest("<html>"),
            Err(DictionaryError::Invalid(_))
        ));
    }
}
