//! Language presets: per-language settings loaded from `data/languages/*.toml`.

use serde::Deserialize;
use std::error::Error;
use std::fs;
use std::path::Path;

/// One language's settings. Field names match the TOML keys in `data/languages`.
#[derive(Debug, Clone, PartialEq, Deserialize)]
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
    /// Dictionary URL with `{term}` as the placeholder for the word.
    pub dictionary_url: String,
    /// Translator URL with `{term}` as the placeholder for the word.
    pub translator_url: String,
}

/// Parses one preset from TOML text.
pub fn parse_preset(text: &str) -> Result<LanguagePreset, toml::de::Error> {
    toml::from_str(text)
}

/// Loads every `*.toml` file in `dir`, in file-name order.
///
/// Returns an error if the directory cannot be read or any file fails to parse.
pub fn load_presets(dir: &Path) -> Result<Vec<LanguagePreset>, Box<dyn Error>> {
    let mut paths: Vec<_> = fs::read_dir(dir)?
        .map(|entry| entry.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    paths.retain(|p| p.extension().is_some_and(|ext| ext == "toml"));
    paths.sort();

    let mut presets = Vec::with_capacity(paths.len());
    for path in paths {
        let text = fs::read_to_string(&path)?;
        let preset = parse_preset(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        presets.push(preset);
    }
    Ok(presets)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn languages_dir() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../data/languages")
    }

    #[test]
    fn loads_every_preset_in_data_languages() {
        let presets = load_presets(&languages_dir()).expect("presets should load");
        let codes: Vec<&str> = presets.iter().map(|p| p.code.as_str()).collect();
        assert_eq!(codes, ["de", "en", "es", "fr"]);
    }

    #[test]
    fn parses_a_minimal_preset() {
        let text = r#"
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
        let preset = parse_preset(text).expect("valid preset");
        assert_eq!(preset.code, "xx");
        assert!(!preset.rtl);
    }

    #[test]
    fn rejects_a_preset_missing_a_field() {
        assert!(parse_preset("name = \"Only a name\"").is_err());
    }
}
