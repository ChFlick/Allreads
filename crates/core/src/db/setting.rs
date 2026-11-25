//! Small app settings stored with the data, e.g. the AnkiDroid ids.
//! Never API keys: those stay in the Android Keystore.

use rusqlite::{params, OptionalExtension};

use super::{Database, DbError};

impl Database {
    /// The value stored under `key`, if any.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn setting(&self, key: &str) -> Result<Option<String>, DbError> {
        Ok(self
            .conn()
            .query_row("SELECT value FROM setting WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()?)
    }

    /// Stores `value` under `key`, replacing any earlier value.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), DbError> {
        self.conn().execute(
            "INSERT INTO setting (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::db::tests_support::test_database;

    #[test]
    fn a_setting_is_stored_and_replaced() {
        let db = test_database();
        assert_eq!(db.setting("anki.deck").unwrap(), None);
        db.set_setting("anki.deck", "1").unwrap();
        db.set_setting("anki.deck", "42").unwrap();
        assert_eq!(db.setting("anki.deck").unwrap().as_deref(), Some("42"));
    }
}
