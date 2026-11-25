//! Writing the dictionary file, in one transaction for speed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use allreads_core::dictionary::{FORMAT_VERSION, INDEXES, SCHEMA};
use rusqlite::{params, Connection, Transaction};

use crate::kaikki::{FormLink, Headword};

/// Counts for the summary line.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub entries: u64,
    pub senses: u64,
    pub forms: u64,
}

/// An open dictionary file being filled.
pub struct Writer<'conn> {
    tx: Transaction<'conn>,
    counts: Counts,
    /// Forms are gathered and merged before writing: Wiktionary describes the
    /// same form both in its own entry and in its lemma's table, with
    /// different tags. Keyed by (key, lemma, pos); each value is a set of tag sets.
    forms: BTreeMap<(String, String, String), BTreeSet<String>>,
}

/// Creates a fresh file at `path` (replacing one that is there).
///
/// # Errors
/// Returns an error if the file cannot be created.
pub fn create(path: &Path) -> rusqlite::Result<Connection> {
    if path.exists() {
        // A half-written file from an earlier run must not be extended.
        std::fs::remove_file(path)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    }
    let conn = Connection::open(path)?;
    // The file is written once and then only read; no journal needed while building.
    conn.execute_batch("PRAGMA journal_mode = OFF; PRAGMA synchronous = OFF;")?;
    conn.execute_batch(SCHEMA)?;
    Ok(conn)
}

impl<'conn> Writer<'conn> {
    /// Starts writing into `conn`.
    ///
    /// # Errors
    /// Returns an error if the transaction cannot start.
    pub fn new(conn: &'conn mut Connection) -> rusqlite::Result<Self> {
        Ok(Writer {
            tx: conn.transaction()?,
            counts: Counts::default(),
            forms: BTreeMap::new(),
        })
    }

    /// Adds a headword and its senses.
    ///
    /// # Errors
    /// Returns an error if an insert fails.
    pub fn add_headword(&mut self, headword: &Headword) -> rusqlite::Result<()> {
        self.tx.execute(
            "INSERT INTO entry (key, word, pos, ipa) VALUES (?1, ?2, ?3, ?4)",
            params![headword.key, headword.word, headword.pos, headword.ipa],
        )?;
        let id = self.tx.last_insert_rowid();
        let mut insert = self
            .tx
            .prepare_cached("INSERT INTO sense (entry_id, position, gloss) VALUES (?1, ?2, ?3)")?;
        for (position, gloss) in (0_i64..).zip(&headword.glosses) {
            insert.execute(params![id, position, gloss])?;
            self.counts.senses += 1;
        }
        self.counts.entries += 1;
        Ok(())
    }

    /// Adds a form link. Links for the same form, lemma and part of speech
    /// are merged when the file is finished.
    pub fn add_form(&mut self, link: &FormLink) {
        self.forms
            .entry((link.key.clone(), link.lemma.clone(), link.pos.clone()))
            .or_default()
            .insert(link.tags.clone());
    }

    /// Writes `meta`, builds the indexes and commits.
    ///
    /// # Errors
    /// Returns an error if a statement fails.
    pub fn finish(mut self, pair: &str, built: &str) -> rusqlite::Result<Counts> {
        {
            let mut insert = self
                .tx
                .prepare("INSERT INTO form (key, lemma, pos, tags) VALUES (?1, ?2, ?3, ?4)")?;
            for ((key, lemma, pos), tag_sets) in &self.forms {
                insert.execute(params![key, lemma, pos, merge_tags(tag_sets)])?;
                self.counts.forms += 1;
            }
        }
        let meta = [
            ("format", FORMAT_VERSION.to_string()),
            ("pair", pair.to_string()),
            (
                "source",
                "Wiktionary via kaikki.org (CC BY-SA 4.0)".to_string(),
            ),
            ("built", built.to_string()),
            ("entries", self.counts.entries.to_string()),
        ];
        for (key, value) in meta {
            self.tx.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)",
                params![key, value],
            )?;
        }
        self.tx.execute_batch(INDEXES)?;
        let counts = self.counts;
        self.tx.commit()?;
        Ok(counts)
    }
}

/// Joins a form's readings with "; ", leaving out any reading whose tags are
/// all contained in another one ("imperative second-person singular" is
/// covered by "imperative present second-person singular").
fn merge_tags(tag_sets: &BTreeSet<String>) -> String {
    let sets: Vec<BTreeSet<&str>> = tag_sets
        .iter()
        .map(|t| t.split(' ').filter(|w| !w.is_empty()).collect())
        .collect();
    let mut kept: Vec<String> = Vec::new();
    for (i, set) in sets.iter().enumerate() {
        let covered = sets
            .iter()
            .enumerate()
            .any(|(j, other)| i != j && set.is_subset(other) && (set != other || j < i));
        if !covered && !set.is_empty() {
            kept.push(set.iter().copied().collect::<Vec<_>>().join(" "));
        }
    }
    kept.join("; ")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sets(tags: &[&str]) -> BTreeSet<String> {
        tags.iter().map(|t| (*t).to_string()).collect()
    }

    #[test]
    fn covered_readings_are_dropped() {
        let merged = merge_tags(&sets(&[
            "imperative second-person singular",
            "imperative present second-person singular",
            "first-person indicative present singular",
            "first-person indicative present second-person singular",
        ]));
        assert_eq!(
            merged,
            "first-person indicative present second-person singular; imperative present second-person singular"
        );
    }

    #[test]
    fn untagged_links_add_nothing() {
        assert_eq!(merge_tags(&sets(&["", "plural"])), "plural");
        assert_eq!(merge_tags(&sets(&[""])), "");
    }
}
