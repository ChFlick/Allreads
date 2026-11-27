//! Offline dictionaries: the file format shared by `dictgen` (which writes
//! it) and the app (which installs files and looks words up in them).
//!
//! - `lookup`: [`Dictionary`], an installed file opened read-only.
//! - `install`: checking a downloaded file and moving it into place.
//! - `manifest`: the list of dictionaries offered for download.

mod common_form;
mod install;
mod lookup;
mod manifest;

pub use common_form::common_form;
pub use install::{file_sha256, install, installed, remove, valid_pair, InstalledDictionary};
pub use lookup::{Dictionary, DictionaryEntry, FormOf, Lookup, PhraseHit, WordKinds};
pub use manifest::{parse_manifest, AvailableDictionary};

/// The tables of a dictionary file. Indexes are created by
/// [`INDEXES`] after the bulk insert, which is much faster.
pub const SCHEMA: &str = include_str!("schema.sql");

/// Indexes for lookups by term key.
/// (`form` needs none: its primary key starts with `key`.)
pub const INDEXES: &str = "CREATE INDEX entry_key ON entry (key);";

/// Optional word-frequency tables for the vocabulary check, added by
/// `dictgen` from a frequency source (its name in `meta` as
/// `frequency_source`). Added without a format bump: older files have none,
/// older apps ignore them.
pub const FREQUENCY_SCHEMA: &str = "
-- Dictionary forms by how common they are: rank 0 is the most common.
CREATE TABLE frequency (
    rank        INTEGER PRIMARY KEY,
    key         TEXT NOT NULL,
    per_million REAL NOT NULL
);
-- The word forms counted under each ranked dictionary form, the dictionary
-- form itself included (\"lire\": \"lire\", \"lisait\", \"lu\", ...).
CREATE TABLE frequency_form (
    rank INTEGER NOT NULL,
    key  TEXT NOT NULL,
    PRIMARY KEY (rank, key)
) WITHOUT ROWID;
";

/// Bumped whenever [`SCHEMA`] changes; stored in `meta` as `format`.
pub const FORMAT_VERSION: u32 = 1;

/// Why a dictionary could not be installed or read.
#[derive(Debug, thiserror::Error)]
pub enum DictionaryError {
    #[error("the download is damaged (checksum does not match)")]
    ChecksumMismatch,
    #[error("not a usable dictionary: {0}")]
    Invalid(String),
    #[error("file error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::Path;

    use rusqlite::{params, Connection};

    use super::{FORMAT_VERSION, INDEXES, SCHEMA};

    /// Writes a tiny French dictionary: lit (bed), lire (to read), and the forms lit and lisait.
    pub fn write_tiny(path: &Path, pair: &str, format: u32) {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(SCHEMA).unwrap();
        conn.execute_batch(
            "INSERT INTO entry (id, key, word, pos, ipa) VALUES
                 (1, 'lit', 'lit', 'noun', '/li/'),
                 (2, 'lire', 'lire', 'verb', '/liʁ/'),
                 (3, 'pierre précieuse', 'pierre précieuse', 'noun', '');
             INSERT INTO sense (entry_id, position, gloss) VALUES
                 (1, 0, 'bed'), (1, 1, 'layer'), (2, 0, 'to read'), (3, 0, 'gemstone');
             INSERT INTO synonym (entry_id, position, word) VALUES
                 (1, 0, 'couche'), (1, 1, 'couchette');
             INSERT INTO form (key, lemma, pos, tags) VALUES
                 ('lit', 'lire', 'verb', 'indicative present singular third-person'),
                 ('lisait', 'lire', 'verb', 'imperfect indicative singular third-person');",
        )
        .unwrap();
        conn.execute_batch(INDEXES).unwrap();
        for (key, value) in [
            ("format", format.to_string()),
            ("pair", pair.to_string()),
            ("built", "2026-11-25".to_string()),
        ] {
            conn.execute(
                "INSERT INTO meta (key, value) VALUES (?1, ?2)",
                params![key, value],
            )
            .unwrap();
        }
    }

    #[test]
    fn the_schema_and_indexes_create_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        write_tiny(&dir.path().join("x.sqlite"), "fr-en", FORMAT_VERSION);
    }
}
