//! Cached translations, so the same text is never paid for twice.

use rusqlite::{params, OptionalExtension};
use unicode_normalization::UnicodeNormalization;

use super::{require_text, Database, DbError};

/// What a cached translation is stored under.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKey {
    /// The text, normalised: NFC, whitespace collapsed. Case is kept, since it
    /// can change a translation.
    pub text: String,
    pub source_lang: String,
    pub target_lang: String,
    /// Text that guided the translation (e.g. the word's sentence), normalised
    /// like `text`; empty for none. A word in two sentences can translate differently.
    pub context: String,
    /// The translation service, e.g. `deepl`.
    pub provider: String,
}

impl CacheKey {
    /// Builds a key, normalising `text` and `context` so the same input always matches.
    #[must_use]
    pub fn new(
        text: &str,
        context: Option<&str>,
        source_lang: &str,
        target_lang: &str,
        provider: &str,
    ) -> Self {
        CacheKey {
            text: normalise(text),
            context: context.map(normalise).unwrap_or_default(),
            source_lang: source_lang.to_lowercase(),
            target_lang: target_lang.to_lowercase(),
            provider: provider.to_string(),
        }
    }

    /// The context, or `None` if there is none.
    #[must_use]
    pub fn context(&self) -> Option<&str> {
        (!self.context.is_empty()).then_some(self.context.as_str())
    }
}

/// NFC, with whitespace collapsed to single spaces. Case is kept.
fn normalise(text: &str) -> String {
    let text: String = text.nfc().collect();
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

impl Database {
    /// Stores a translation, replacing any earlier one for the same key.
    ///
    /// # Errors
    /// Returns `InvalidValue` for blank text, and a database error if the
    /// statement fails.
    pub fn save_translation(&self, key: &CacheKey, translation: &str) -> Result<(), DbError> {
        require_text("text to translate", &key.text)?;
        self.conn().execute(
            "INSERT INTO translation_cache
                 (source_text, source_lang, target_lang, context, provider, translation)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT (source_text, source_lang, target_lang, context, provider)
             DO UPDATE SET translation = excluded.translation",
            params![
                key.text,
                key.source_lang,
                key.target_lang,
                key.context,
                key.provider,
                translation
            ],
        )?;
        Ok(())
    }

    /// The cached translation for this key, if there is one.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn cached_translation(&self, key: &CacheKey) -> Result<Option<String>, DbError> {
        Ok(self
            .conn()
            .query_row(
                "SELECT translation FROM translation_cache
                 WHERE source_text = ?1 AND source_lang = ?2 AND target_lang = ?3
                   AND context = ?4 AND provider = ?5",
                params![
                    key.text,
                    key.source_lang,
                    key.target_lang,
                    key.context,
                    key.provider
                ],
                |row| row.get(0),
            )
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests_support::test_database;

    fn key(text: &str, target: &str, provider: &str) -> CacheKey {
        CacheKey::new(text, None, "fr", target, provider)
    }

    #[test]
    fn missing_translation_is_none() {
        let db = test_database();
        assert_eq!(
            db.cached_translation(&key("Bonjour.", "en", "deepl"))
                .unwrap(),
            None
        );
    }

    #[test]
    fn saved_translation_is_found_again_after_normalising() {
        let db = test_database();
        db.save_translation(&key("Le  chat\n dort.", "en", "deepl"), "The cat sleeps.")
            .unwrap();
        assert_eq!(
            db.cached_translation(&key("Le chat dort.", "en", "deepl"))
                .unwrap(),
            Some("The cat sleeps.".to_string())
        );
    }

    #[test]
    fn target_languages_do_not_collide() {
        let db = test_database();
        db.save_translation(&key("chat", "en", "deepl"), "cat")
            .unwrap();
        db.save_translation(&key("chat", "de", "deepl"), "Katze")
            .unwrap();
        assert_eq!(
            db.cached_translation(&key("chat", "en", "deepl"))
                .unwrap()
                .as_deref(),
            Some("cat")
        );
        assert_eq!(
            db.cached_translation(&key("chat", "de", "deepl"))
                .unwrap()
                .as_deref(),
            Some("Katze")
        );
    }

    #[test]
    fn the_same_word_in_different_sentences_is_cached_separately() {
        let db = test_database();
        let reads = CacheKey::new("lit", Some("Il lit un livre."), "fr", "en", "deepl");
        let bed = CacheKey::new("lit", Some("Le lit  est grand."), "fr", "en", "deepl");
        db.save_translation(&reads, "reads").unwrap();
        db.save_translation(&bed, "bed").unwrap();
        assert_eq!(
            db.cached_translation(&reads).unwrap().as_deref(),
            Some("reads")
        );
        assert_eq!(db.cached_translation(&bed).unwrap().as_deref(), Some("bed"));
        assert_eq!(
            db.cached_translation(&key("lit", "en", "deepl")).unwrap(),
            None
        );
        assert_eq!(bed.context(), Some("Le lit est grand."));
        assert_eq!(key("lit", "en", "deepl").context(), None);
    }

    #[test]
    fn providers_are_cached_separately() {
        let db = test_database();
        db.save_translation(&key("chat", "en", "deepl"), "cat")
            .unwrap();
        assert_eq!(
            db.cached_translation(&key("chat", "en", "mymemory"))
                .unwrap(),
            None
        );
    }

    #[test]
    fn case_is_kept_in_the_key() {
        assert_ne!(key("Est-ce", "en", "deepl"), key("est-ce", "en", "deepl"));
        assert_eq!(key("x", "EN", "deepl").target_lang, "en");
    }

    #[test]
    fn blank_text_is_refused() {
        let db = test_database();
        assert!(matches!(
            db.save_translation(&key(" ", "en", "deepl"), "x"),
            Err(DbError::InvalidValue(_))
        ));
    }
}
