//! Language presets: per-language settings loaded from `data/languages/*.toml`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

/// One language's settings. Field names match the TOML keys in `data/languages`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct LanguagePreset {
    pub name: String,
    pub code: String,
    /// Regex character-class contents, e.g. `a-zA-Z`.
    pub word_characters: String,
    /// Characters that can end a sentence, e.g. `.!?`.
    pub sentence_end_chars: String,
    /// Pipe-separated exceptions to sentence splitting, e.g. `Mr.|Dr.`. May be empty.
    pub sentence_exceptions: String,
    /// Pipe-separated `from=to` pairs, e.g. `'=’`. May be empty.
    pub character_substitutions: String,
    pub rtl: bool,
    pub remove_spaces: bool,
    pub split_each_char: bool,
    pub tts_locale: String,
    /// Dictionary URL with `{term}` as the placeholder for the word. May be empty.
    pub dictionary_url: String,
    /// Translator URL with `{term}` as the placeholder for the word. May be empty.
    pub translator_url: String,
}

/// Errors from reading, parsing or checking presets.
#[derive(Debug, Error)]
pub enum PresetError {
    #[error("could not read {}: {source}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not parse {}: {source}", path.display())]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },
    #[error("invalid preset {}: {reason}", path.display())]
    Invalid { path: PathBuf, reason: String },
    #[error("two presets use the code '{0}'")]
    DuplicateCode(String),
}

/// The presets in `data/languages`, compiled into the library. Android has no
/// such folder at runtime, so the app reads these instead.
const BUILTIN: &[(&str, &str)] = &[
    ("de.toml", include_str!("../../../data/languages/de.toml")),
    ("en.toml", include_str!("../../../data/languages/en.toml")),
    ("es.toml", include_str!("../../../data/languages/es.toml")),
    ("fr.toml", include_str!("../../../data/languages/fr.toml")),
];

/// Parses and checks the built-in presets.
///
/// # Errors
/// Returns a `PresetError` if a built-in preset is invalid. The tests make sure
/// this never happens in a release.
pub fn builtin_presets() -> Result<Vec<LanguagePreset>, PresetError> {
    let mut presets = Vec::with_capacity(BUILTIN.len());
    for (name, text) in BUILTIN {
        let path = PathBuf::from(format!("builtin/{name}"));
        let preset = parse_preset(text).map_err(|source| PresetError::Parse {
            path: path.clone(),
            source,
        })?;
        preset
            .validate()
            .map_err(|reason| PresetError::Invalid { path, reason })?;
        presets.push(preset);
    }
    ensure_unique_codes(&presets)?;
    Ok(presets)
}

/// Parses one preset from TOML text, without checking its values.
///
/// # Errors
/// Returns the TOML error if the text is not valid or a required field is missing.
pub fn parse_preset(text: &str) -> Result<LanguagePreset, toml::de::Error> {
    toml::from_str(text)
}

impl LanguagePreset {
    /// Checks that the values are usable. Returns the first problem found.
    ///
    /// # Errors
    /// Returns a human-readable reason for the first problem found.
    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("name is empty".into());
        }
        let code_ok =
            (2..=3).contains(&self.code.len()) && self.code.chars().all(|c| c.is_ascii_lowercase());
        if !code_ok {
            return Err(format!(
                "code '{}' must be 2 or 3 lowercase letters",
                self.code
            ));
        }
        if self.word_characters.is_empty() {
            return Err("word_characters is empty".into());
        }
        if self.tts_locale.trim().is_empty() {
            return Err("tts_locale is empty".into());
        }
        check_url("dictionary_url", &self.dictionary_url)?;
        check_url("translator_url", &self.translator_url)?;
        Ok(())
    }
}

/// An empty URL means "none". Otherwise it must be http(s) and contain `{term}`.
fn check_url(field: &str, url: &str) -> Result<(), String> {
    if url.is_empty() {
        return Ok(());
    }
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(format!("{field} must start with http:// or https://"));
    }
    if !url.contains("{term}") {
        return Err(format!("{field} must contain {{term}}"));
    }
    Ok(())
}

/// Loads every `*.toml` file in `dir`, in file-name order, and checks them.
///
/// Fails on the first unreadable, unparsable or invalid file, and if two
/// presets share a code.
///
/// # Errors
/// Returns a `PresetError` describing the first file or rule that fails.
pub fn load_presets(dir: &Path) -> Result<Vec<LanguagePreset>, PresetError> {
    let entries = fs::read_dir(dir).map_err(|source| PresetError::Read {
        path: dir.to_path_buf(),
        source,
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry
            .map_err(|source| PresetError::Read {
                path: dir.to_path_buf(),
                source,
            })?
            .path();
        if path.extension().is_some_and(|ext| ext == "toml") {
            paths.push(path);
        }
    }
    paths.sort();

    let mut presets = Vec::with_capacity(paths.len());
    for path in paths {
        let text = fs::read_to_string(&path).map_err(|source| PresetError::Read {
            path: path.clone(),
            source,
        })?;
        let preset = parse_preset(&text).map_err(|source| PresetError::Parse {
            path: path.clone(),
            source,
        })?;
        preset.validate().map_err(|reason| PresetError::Invalid {
            path: path.clone(),
            reason,
        })?;
        presets.push(preset);
    }
    ensure_unique_codes(&presets)?;
    Ok(presets)
}

/// Fails if any two presets share a code.
///
/// # Errors
/// Returns `DuplicateCode` naming the first repeated code.
pub fn ensure_unique_codes(presets: &[LanguagePreset]) -> Result<(), PresetError> {
    let mut seen = std::collections::HashSet::new();
    for preset in presets {
        if !seen.insert(preset.code.as_str()) {
            return Err(PresetError::DuplicateCode(preset.code.clone()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn languages_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/languages")
    }

    const VALID: &str = r#"
        name = "Test"
        code = "xx"
        word_characters = "a-z"
        sentence_end_chars = "."
        sentence_exceptions = ""
        character_substitutions = ""
        rtl = false
        remove_spaces = false
        split_each_char = false
        tts_locale = "xx-XX"
        dictionary_url = "https://example.org/{term}"
        translator_url = ""
    "#;

    fn valid_preset() -> LanguagePreset {
        parse_preset(VALID).expect("test preset parses")
    }

    #[test]
    fn loads_every_preset_in_data_languages() {
        let presets = load_presets(&languages_dir()).expect("presets should load");
        let codes: Vec<&str> = presets.iter().map(|p| p.code.as_str()).collect();
        assert_eq!(codes, ["de", "en", "es", "fr"]);
    }

    #[test]
    fn builtin_presets_match_every_file_in_data_languages() {
        let from_disk = load_presets(&languages_dir()).expect("presets load from disk");
        let builtin = builtin_presets().expect("built-in presets are valid");
        assert_eq!(builtin, from_disk, "add new preset files to BUILTIN too");
    }

    #[test]
    fn parses_a_minimal_preset() {
        let preset = valid_preset();
        assert_eq!(preset.code, "xx");
        assert!(!preset.rtl);
        assert_eq!(preset.validate(), Ok(()));
    }

    #[test]
    fn rejects_a_preset_missing_a_field() {
        assert!(parse_preset("name = \"Only a name\"").is_err());
    }

    #[test]
    fn rejects_a_bad_code() {
        for code in ["X", "english", "F1", ""] {
            let mut preset = valid_preset();
            preset.code = code.to_string();
            assert!(
                preset.validate().is_err(),
                "code {code:?} should be rejected"
            );
        }
    }

    #[test]
    fn rejects_an_empty_word_character_class() {
        let mut preset = valid_preset();
        preset.word_characters.clear();
        assert_eq!(
            preset.validate(),
            Err("word_characters is empty".to_string())
        );
    }

    #[test]
    fn rejects_a_dictionary_url_without_the_placeholder() {
        let mut preset = valid_preset();
        preset.dictionary_url = "https://example.org/search".into();
        assert!(preset.validate().unwrap_err().contains("{term}"));
    }

    #[test]
    fn rejects_a_url_without_a_web_scheme() {
        let mut preset = valid_preset();
        preset.translator_url = "ftp://example.org/{term}".into();
        assert!(preset.validate().unwrap_err().contains("http"));
    }

    #[test]
    fn empty_urls_are_allowed() {
        let preset = valid_preset();
        assert!(preset.translator_url.is_empty());
        assert_eq!(preset.validate(), Ok(()));
    }

    #[test]
    fn duplicate_codes_are_rejected() {
        let presets = vec![valid_preset(), valid_preset()];
        assert!(matches!(
            ensure_unique_codes(&presets),
            Err(PresetError::DuplicateCode(code)) if code == "xx"
        ));
    }

    #[test]
    fn missing_directory_is_a_read_error() {
        let missing = Path::new("no/such/dir");
        assert!(matches!(
            load_presets(missing),
            Err(PresetError::Read { .. })
        ));
    }
}
