//! Builds Allreads' offline dictionary files from kaikki.org's Wiktionary
//! extracts. The file format is defined in `allreads_core::dictionary`.

pub mod kaikki;
pub mod writer;

use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use allreads_core::text::LanguageRules;
use flate2::read::GzDecoder;

use crate::kaikki::{convert, KaikkiEntry};
use crate::writer::{Counts, Writer};

/// Reads the extract at `input` (plain or `.gz`), keeps entries in
/// `language`, and writes the dictionary to `output`.
///
/// # Errors
/// Returns a message for a file that cannot be read or written, or a line
/// that is not JSON.
pub fn build(
    input: &Path,
    output: &Path,
    language: &str,
    pair: &str,
    built: &str,
) -> Result<Counts, String> {
    let preset = allreads_core::presets::builtin_presets()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|p| p.code == language)
        .ok_or_else(|| format!("no built-in preset for language {language}"))?;
    let rules = LanguageRules::new(&preset)?;

    let file = File::open(input).map_err(|e| format!("{}: {e}", input.display()))?;
    let reader: Box<dyn Read> = if input.extension().is_some_and(|e| e == "gz") {
        Box::new(GzDecoder::new(file))
    } else {
        Box::new(file)
    };

    let mut conn = writer::create(output).map_err(|e| e.to_string())?;
    let mut writer = Writer::new(&mut conn).map_err(|e| e.to_string())?;
    for (number, line) in BufReader::new(reader).lines().enumerate() {
        let line = line.map_err(|e| format!("line {}: {e}", number + 1))?;
        if line.trim().is_empty() {
            continue;
        }
        let entry: KaikkiEntry =
            serde_json::from_str(&line).map_err(|e| format!("line {}: {e}", number + 1))?;
        if entry.lang_code != language {
            continue;
        }
        let (headword, links) = convert(&entry, &rules);
        if let Some(headword) = headword {
            writer.add_headword(&headword).map_err(|e| e.to_string())?;
        }
        for link in &links {
            writer.add_form(link);
        }
    }
    writer.finish(pair, built).map_err(|e| e.to_string())
}
