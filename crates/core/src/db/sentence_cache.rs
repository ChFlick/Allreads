//! Cached sentence translations and AI explanations, keyed by sentence hash
//! and provider, so the same sentence is never paid for twice.

use rusqlite::{params, OptionalExtension, Row};

use super::{require_text, Database, DbError};

/// What is cached for one sentence and provider. Either part may be missing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CachedSentence {
    pub translation: Option<String>,
    pub explanation: Option<String>,
}

impl Database {
    /// Stores a translation, keeping any explanation already cached.
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn save_translation(
        &self,
        sentence_hash: &str,
        provider: &str,
        translation: &str,
    ) -> Result<(), DbError> {
        require_text("sentence hash", sentence_hash)?;
        self.conn().execute(
            "INSERT INTO sentence_cache (sentence_hash, provider, translation)
             VALUES (?1, ?2, ?3)
             ON CONFLICT (sentence_hash, provider) DO UPDATE SET translation = excluded.translation",
            params![sentence_hash, provider, translation],
        )?;
        Ok(())
    }

    /// Stores an explanation, keeping any translation already cached.
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn save_explanation(
        &self,
        sentence_hash: &str,
        provider: &str,
        explanation: &str,
    ) -> Result<(), DbError> {
        require_text("sentence hash", sentence_hash)?;
        self.conn().execute(
            "INSERT INTO sentence_cache (sentence_hash, provider, explanation)
             VALUES (?1, ?2, ?3)
             ON CONFLICT (sentence_hash, provider) DO UPDATE SET explanation = excluded.explanation",
            params![sentence_hash, provider, explanation],
        )?;
        Ok(())
    }

    /// What is cached for this sentence and provider, if anything.
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn cached_sentence(
        &self,
        sentence_hash: &str,
        provider: &str,
    ) -> Result<Option<CachedSentence>, DbError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT translation, explanation FROM sentence_cache
                 WHERE sentence_hash = ?1 AND provider = ?2",
                params![sentence_hash, provider],
                cached_from_row,
            )
            .optional()?)
    }
}

fn cached_from_row(row: &Row) -> rusqlite::Result<CachedSentence> {
    Ok(CachedSentence {
        translation: row.get(0)?,
        explanation: row.get(1)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests_support::test_database;

    #[test]
    fn missing_sentence_is_none() {
        let db = test_database();
        assert_eq!(db.cached_sentence("abc", "deepl").unwrap(), None);
    }

    #[test]
    fn translation_and_explanation_are_kept_side_by_side() {
        let db = test_database();
        db.save_translation("abc", "deepl", "The cat sleeps.")
            .unwrap();
        db.save_explanation("abc", "deepl", "Present tense.")
            .unwrap();
        assert_eq!(
            db.cached_sentence("abc", "deepl").unwrap(),
            Some(CachedSentence {
                translation: Some("The cat sleeps.".into()),
                explanation: Some("Present tense.".into()),
            })
        );
    }

    #[test]
    fn providers_are_cached_separately() {
        let db = test_database();
        db.save_translation("abc", "deepl", "one").unwrap();
        db.save_translation("abc", "libre", "two").unwrap();
        let deepl = db.cached_sentence("abc", "deepl").unwrap().unwrap();
        assert_eq!(deepl.translation.as_deref(), Some("one"));
    }

    #[test]
    fn blank_hash_is_refused() {
        let db = test_database();
        assert!(matches!(
            db.save_translation(" ", "deepl", "x"),
            Err(DbError::InvalidValue(_))
        ));
    }
}
