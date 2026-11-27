//! Word frequencies for the vocabulary check, from pluggable sources.
//!
//! A [`FrequencySource`] reads its own file format into plain records: a word
//! form, how common it is, and (if the source knows it) its dictionary form.
//! [`add_frequencies`] turns those records into the same tables whatever the
//! source, in a dictionary that is already built, so the app never depends on
//! the provider. Adding a provider means one more `FrequencySource`.

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use allreads_core::text::LanguageRules;
use rusqlite::{params, Connection};

/// How many dictionary forms are kept, most common first: well past the
/// rarest band the check asks about.
const MAX_RANKED: usize = 50_000;

/// One word form from a frequency list.
#[derive(Debug, Clone, PartialEq)]
pub struct FrequencyRecord {
    pub form: String,
    /// Occurrences per million words.
    pub per_million: f64,
    /// Its dictionary form, if the source says ("lisait" → "lire").
    pub lemma: Option<String>,
}

/// A provider of word frequencies.
pub trait FrequencySource {
    /// Its name and licence, stored in the dictionary and credited in the app.
    fn name(&self) -> String;

    /// Reads the provider's file.
    ///
    /// # Errors
    /// Returns a message for a file that cannot be read or is not in the
    /// provider's format.
    fn read(&self, path: &Path) -> Result<Vec<FrequencyRecord>, String>;
}

/// The source named on the command line: `lexique` or `frequencywords`.
#[must_use]
pub fn source_named(name: &str) -> Option<Box<dyn FrequencySource>> {
    match name.to_lowercase().as_str() {
        "lexique" => Some(Box::new(Lexique)),
        "frequencywords" => Some(Box::new(FrequencyWords)),
        _ => None,
    }
}

/// Lexique 3 (French; New, Pallier et al., CC BY-SA 4.0): a tab-separated
/// table with a header, one row per form and dictionary form. Its frequencies
/// from books and from film subtitles are averaged.
pub struct Lexique;

impl FrequencySource for Lexique {
    fn name(&self) -> String {
        "Lexique 3 (New, Pallier et al.; CC BY-SA 4.0)".into()
    }

    fn read(&self, path: &Path) -> Result<Vec<FrequencyRecord>, String> {
        let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut lines = BufReader::new(file).lines();
        let header = lines
            .next()
            .ok_or("an empty Lexique file")?
            .map_err(|e| e.to_string())?;
        let columns: Vec<&str> = header.split('\t').collect();
        let column = |name: &str| {
            columns
                .iter()
                .position(|c| *c == name)
                .ok_or_else(|| format!("Lexique has no column {name}"))
        };
        let (form, lemma, books, films) = (
            column("ortho")?,
            column("lemme")?,
            column("freqlivres")?,
            column("freqfilms2")?,
        );
        let mut records = Vec::new();
        for (number, line) in lines.enumerate() {
            let line = line.map_err(|e| format!("line {}: {e}", number + 2))?;
            let fields: Vec<&str> = line.split('\t').collect();
            let field = |i: usize| fields.get(i).copied().unwrap_or("");
            let number_at = |i: usize| field(i).trim().parse::<f64>().unwrap_or(0.0);
            records.push(FrequencyRecord {
                form: field(form).to_string(),
                per_million: (number_at(books) + number_at(films)) / 2.0,
                lemma: Some(field(lemma).to_string()).filter(|l| !l.is_empty()),
            });
        }
        Ok(records)
    }
}

/// `FrequencyWords` (Hermit Dave; from `OpenSubtitles` 2018; CC BY-SA 4.0):
/// "word count" per line, forms only. Dictionary forms come from the
/// dictionary's own "form of" links.
pub struct FrequencyWords;

impl FrequencySource for FrequencyWords {
    fn name(&self) -> String {
        "FrequencyWords (Hermit Dave, from OpenSubtitles 2018; CC BY-SA 4.0)".into()
    }

    fn read(&self, path: &Path) -> Result<Vec<FrequencyRecord>, String> {
        let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut counts = Vec::new();
        for (number, line) in BufReader::new(file).lines().enumerate() {
            let line = line.map_err(|e| format!("line {}: {e}", number + 1))?;
            let Some((word, count)) = line.trim().rsplit_once(' ') else {
                continue;
            };
            let count: f64 = count
                .parse()
                .map_err(|_| format!("line {}: no count in {line:?}", number + 1))?;
            counts.push((word.to_string(), count));
        }
        let total: f64 = counts.iter().map(|(_, count)| count).sum();
        Ok(counts
            .into_iter()
            .map(|(form, count)| FrequencyRecord {
                form,
                per_million: if total > 0.0 {
                    count / total * 1_000_000.0
                } else {
                    0.0
                },
                lemma: None,
            })
            .collect())
    }
}

/// What [`add_frequencies`] did, for the log.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct FrequencySummary {
    /// Dictionary forms ranked.
    pub ranked: usize,
    /// Word forms counted under them.
    pub forms: usize,
    /// Records left out: not words the dictionary knows (names, typos).
    pub skipped: usize,
}

/// Adds `source`'s frequencies from `input` to the built dictionary at
/// `dictionary`: its dictionary forms ranked by how common they are (the
/// frequencies of their forms added up), and the forms counted under each.
/// Only words the dictionary knows are kept, which also leaves out names and
/// typos; forms with no dictionary form from the source get one from the
/// dictionary's "form of" links.
///
/// # Errors
/// Returns a message if the language has no preset or a file cannot be read
/// or written.
pub fn add_frequencies(
    dictionary: &Path,
    source: &dyn FrequencySource,
    input: &Path,
    language: &str,
) -> Result<FrequencySummary, String> {
    let preset = allreads_core::presets::builtin_presets()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|p| p.code == language)
        .ok_or_else(|| format!("no built-in preset for language {language}"))?;
    let rules = LanguageRules::new(&preset)?;
    let records = source.read(input)?;

    let mut conn = Connection::open(dictionary).map_err(|e| e.to_string())?;
    let entries = keys(&conn, "SELECT DISTINCT key FROM entry")?;
    let form_lemmas = form_links(&conn)?;

    // Each dictionary form's total frequency, and the forms counted under it.
    let mut totals: HashMap<String, f64> = HashMap::new();
    let mut forms: HashMap<String, HashSet<String>> = HashMap::new();
    let mut summary = FrequencySummary::default();
    for record in records {
        let form = rules.term_key(&record.form);
        // The source's dictionary form when the dictionary has it (so the
        // check only asks words it can look up), else the dictionary's own.
        let from_source = record
            .lemma
            .as_ref()
            .map(|l| rules.term_key(l))
            .filter(|l| entries.contains(l));
        let lemma = from_source.or_else(|| {
            if entries.contains(&form) {
                Some(form.clone())
            } else {
                form_lemmas.get(&form).and_then(|lemmas| {
                    // A form of several words counts under one that has an entry.
                    lemmas
                        .iter()
                        .find(|l| entries.contains(*l))
                        .or(lemmas.first())
                        .cloned()
                })
            }
        });
        let known = entries.contains(&form) || form_lemmas.contains_key(&form);
        let Some(lemma) = lemma.filter(|l| known && is_word(l) && is_word(&form)) else {
            summary.skipped += 1;
            continue;
        };
        *totals.entry(lemma.clone()).or_default() += record.per_million;
        let counted = forms.entry(lemma.clone()).or_default();
        counted.insert(lemma);
        counted.insert(form);
    }

    let mut ranked: Vec<(String, f64)> = totals
        .into_iter()
        .filter(|(_, total)| *total > 0.0)
        .collect();
    // Most common first; ties by key, so the same input gives the same file.
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    ranked.truncate(MAX_RANKED);

    let tx = conn.transaction().map_err(|e| e.to_string())?;
    {
        tx.execute_batch("DROP TABLE IF EXISTS frequency; DROP TABLE IF EXISTS frequency_form;")
            .map_err(|e| e.to_string())?;
        tx.execute_batch(allreads_core::dictionary::FREQUENCY_SCHEMA)
            .map_err(|e| e.to_string())?;
        let mut rank_insert = tx
            .prepare("INSERT INTO frequency (rank, key, per_million) VALUES (?1, ?2, ?3)")
            .map_err(|e| e.to_string())?;
        let mut form_insert = tx
            .prepare("INSERT INTO frequency_form (rank, key) VALUES (?1, ?2)")
            .map_err(|e| e.to_string())?;
        for (rank, (lemma, per_million)) in ranked.iter().enumerate() {
            // At most MAX_RANKED, so it fits; SQLite stores signed integers.
            let rank = i64::try_from(rank).unwrap_or(i64::MAX);
            rank_insert
                .execute(params![rank, lemma, per_million])
                .map_err(|e| e.to_string())?;
            summary.ranked += 1;
            let mut lemma_forms: Vec<&String> = forms
                .get(lemma)
                .map(|f| f.iter().collect())
                .unwrap_or_default();
            lemma_forms.sort();
            for form in lemma_forms {
                form_insert
                    .execute(params![rank, form])
                    .map_err(|e| e.to_string())?;
                summary.forms += 1;
            }
        }
        tx.execute(
            "INSERT OR REPLACE INTO meta (key, value) VALUES ('frequency_source', ?1)",
            [source.name()],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    conn.execute_batch("VACUUM").map_err(|e| e.to_string())?;
    Ok(summary)
}

/// A word worth ranking: letters, with an apostrophe or hyphen inside at most.
fn is_word(key: &str) -> bool {
    key.chars().any(char::is_alphabetic)
        && key
            .chars()
            .all(|c| c.is_alphabetic() || c == '\'' || c == '-')
}

fn keys(conn: &Connection, sql: &str) -> Result<HashSet<String>, String> {
    let mut statement = conn.prepare(sql).map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<_, _>>().map_err(|e| e.to_string())
}

/// The dictionary's "form of" links: each form's dictionary forms.
fn form_links(conn: &Connection) -> Result<HashMap<String, Vec<String>>, String> {
    let mut statement = conn
        .prepare("SELECT key, lemma FROM form ORDER BY key, lemma")
        .map_err(|e| e.to_string())?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| e.to_string())?;
    let mut links: HashMap<String, Vec<String>> = HashMap::new();
    for row in rows {
        let (form, lemma) = row.map_err(|e| e.to_string())?;
        links.entry(form).or_default().push(lemma);
    }
    Ok(links)
}

#[cfg(test)]
mod tests;
