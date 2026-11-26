//! `SQLite` storage: schema migrations and one repository module per table group.
//!
//! Every connection turns on foreign keys, so deleting a row cascades or is
//! refused as the schema says. Repositories take `&Database` and return plain
//! structs, never `rusqlite` rows.

pub mod anki;
pub mod book;
pub mod chapter_cache;
pub mod language;
pub mod progress;
pub mod setting;
pub mod stats;
pub mod term;
pub mod translation_cache;

use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use crate::text::LanguageRules;
use rusqlite::Connection;
use thiserror::Error;

mod migrations {
    refinery::embed_migrations!("migrations");
}

/// Errors from the storage layer.
#[derive(Debug, Error)]
pub enum DbError {
    #[error("database error: {0}")]
    Sql(#[from] rusqlite::Error),
    #[error("could not migrate the database: {0}")]
    Migration(#[from] refinery::Error),
    #[error("{what} not found")]
    NotFound { what: String },
    #[error("invalid value: {0}")]
    InvalidValue(String),
}

/// The schema version after all migrations. Bump it with each new migration.
pub const SCHEMA_VERSION: i64 = 13;

/// An open `SQLite` database with the schema applied.
pub struct Database {
    conn: Connection,
    /// Compiled word and sentence rules per language code, so keying a word
    /// does not compile regexes every time. Cleared when a language changes.
    rules: RefCell<HashMap<String, Arc<LanguageRules>>>,
}

impl Database {
    /// Opens (or creates) the database file at `path` and migrates it.
    /// # Errors
    /// Returns a database error if the file cannot be opened or migrated.
    pub fn open(path: &Path) -> Result<Self, DbError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Opens a fresh database in memory. Used by tests.
    /// # Errors
    /// Returns a database error if the database cannot be created or migrated.
    pub fn open_in_memory() -> Result<Self, DbError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut conn: Connection) -> Result<Self, DbError> {
        conn.pragma_update(None, "foreign_keys", true)?;
        migrations::migrations::runner().run(&mut conn)?;
        Ok(Self {
            conn,
            rules: RefCell::new(HashMap::new()),
        })
    }

    /// The compiled rules for a language, or `None` if it is not stored or its
    /// rules do not compile.
    pub(crate) fn rules(&self, language_code: &str) -> Result<Option<Arc<LanguageRules>>, DbError> {
        if let Some(rules) = self.rules.borrow().get(language_code) {
            return Ok(Some(Arc::clone(rules)));
        }
        let Some(language) = self.language(language_code)? else {
            return Ok(None);
        };
        let Ok(rules) = LanguageRules::new(&language) else {
            return Ok(None);
        };
        let rules = Arc::new(rules);
        self.rules
            .borrow_mut()
            .insert(language_code.to_string(), Arc::clone(&rules));
        Ok(Some(rules))
    }

    /// Forgets the compiled rules, after a language was added or changed.
    pub(crate) fn forget_rules(&self) {
        self.rules.borrow_mut().clear();
    }

    /// The underlying connection, for the repository modules.
    pub(crate) fn conn(&self) -> &Connection {
        &self.conn
    }

    /// The highest migration version applied to this database.
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn schema_version(&self) -> Result<i64, DbError> {
        let version = self.conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM refinery_schema_history",
            [],
            |row| row.get(0),
        )?;
        Ok(version)
    }
}

/// Rejects blank text for a field that must have content.
pub(crate) fn require_text(field: &str, value: &str) -> Result<(), DbError> {
    if value.trim().is_empty() {
        return Err(DbError::InvalidValue(format!("{field} is empty")));
    }
    Ok(())
}

/// Turns "no rows changed" into `NotFound`, so callers can tell a missing row apart.
pub(crate) fn require_one_changed(
    changed: usize,
    what: impl FnOnce() -> String,
) -> Result<(), DbError> {
    if changed == 0 {
        return Err(DbError::NotFound { what: what() });
    }
    Ok(())
}

/// Reads an INTEGER column as `u32`, failing cleanly if the value is out of range.
pub(crate) fn u32_column(row: &rusqlite::Row, index: usize) -> rusqlite::Result<u32> {
    let value: i64 = row.get(index)?;
    u32::try_from(value).map_err(|_| rusqlite::Error::IntegralValueOutOfRange(index, value))
}

/// Shared helpers for the repository tests.
#[cfg(test)]
pub(crate) mod tests_support {
    use super::Database;
    use crate::presets::parse_preset;

    /// A fresh in-memory database with the schema applied and no rows.
    pub(crate) fn test_database() -> Database {
        Database::open_in_memory().expect("in-memory database opens")
    }

    /// An in-memory database with French stored as a language.
    pub(crate) fn french_db() -> Database {
        let db = test_database();
        let french = parse_preset(
            r#"
            name = "French"
            code = "fr"
            word_characters = "a-zA-ZÀ-ÖØ-öø-ȳ"
            sentence_end_chars = ".!?"
            sentence_exceptions = "Dr.|Mr."
            character_substitutions = "´='|`='|’='|‘='"
            rtl = false
            remove_spaces = false
            split_each_char = false
            tts_locale = "fr-FR"
            dictionary_url = ""
            translator_url = ""
            "#,
        )
        .expect("french preset parses");
        db.upsert_language(&french).expect("french is stored");
        db
    }
}

#[cfg(test)]
mod tests {
    use super::tests_support::test_database;
    use super::*;

    #[test]
    fn migrations_apply_to_a_new_database() {
        assert_eq!(test_database().schema_version().unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn data_survives_closing_and_reopening_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("allreads.db");
        {
            let db = Database::open(&path).unwrap();
            db.upsert_language(
                &crate::presets::parse_preset(
                    r#"
                name = "French"
                code = "fr"
                word_characters = "a-z"
                sentence_end_chars = "."
                sentence_exceptions = ""
                character_substitutions = ""
                rtl = false
                remove_spaces = false
                split_each_char = false
                tts_locale = "fr-FR"
                dictionary_url = ""
                translator_url = ""
                "#,
                )
                .unwrap(),
            )
            .unwrap();
        }
        let reopened = Database::open(&path).unwrap();
        assert!(reopened.language("fr").unwrap().is_some());
        assert_eq!(reopened.schema_version().unwrap(), SCHEMA_VERSION);
    }

    #[test]
    fn require_text_refuses_blank_input() {
        assert!(require_text("title", "  \t ").is_err());
        assert!(require_text("title", "Un").is_ok());
    }
}
