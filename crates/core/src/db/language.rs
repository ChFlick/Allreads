//! Language settings, stored from presets so books can refer to them.

use rusqlite::{params, OptionalExtension, Row};

use super::{Database, DbError};
use crate::presets::LanguagePreset;

const COLUMNS: &str = "code, name, word_characters, sentence_end_chars, sentence_exceptions, \
     character_substitutions, rtl, remove_spaces, split_each_char, tts_locale, \
     dictionary_url, translator_url";

impl Database {
    /// Inserts a language, or replaces the settings of one with the same code.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn upsert_language(&self, language: &LanguagePreset) -> Result<(), DbError> {
        self.conn().execute(
            "INSERT INTO language (code, name, word_characters, sentence_end_chars,
                 sentence_exceptions, character_substitutions, rtl, remove_spaces,
                 split_each_char, tts_locale, dictionary_url, translator_url)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT (code) DO UPDATE SET
                 name = excluded.name,
                 word_characters = excluded.word_characters,
                 sentence_end_chars = excluded.sentence_end_chars,
                 sentence_exceptions = excluded.sentence_exceptions,
                 character_substitutions = excluded.character_substitutions,
                 rtl = excluded.rtl,
                 remove_spaces = excluded.remove_spaces,
                 split_each_char = excluded.split_each_char,
                 tts_locale = excluded.tts_locale,
                 dictionary_url = excluded.dictionary_url,
                 translator_url = excluded.translator_url",
            params![
                language.code,
                language.name,
                language.word_characters,
                language.sentence_end_chars,
                language.sentence_exceptions,
                language.character_substitutions,
                language.rtl,
                language.remove_spaces,
                language.split_each_char,
                language.tts_locale,
                language.dictionary_url,
                language.translator_url,
            ],
        )?;
        Ok(())
    }

    /// Stores a language only if its code is not stored yet, so settings the
    /// user changed later are kept.
    ///
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn add_language_if_missing(&self, language: &LanguagePreset) -> Result<(), DbError> {
        if self.language(&language.code)?.is_none() {
            self.upsert_language(language)?;
        }
        Ok(())
    }

    /// The language with this code, if it has been stored.
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn language(&self, code: &str) -> Result<Option<LanguagePreset>, DbError> {
        let sql = format!("SELECT {COLUMNS} FROM language WHERE code = ?1");
        Ok(self
            .conn()
            .query_row(&sql, [code], language_from_row)
            .optional()?)
    }

    /// All stored languages, sorted by code.
    /// # Errors
    /// Returns a database error if the statement fails.
    pub fn languages(&self) -> Result<Vec<LanguagePreset>, DbError> {
        let sql = format!("SELECT {COLUMNS} FROM language ORDER BY code");
        let mut statement = self.conn().prepare(&sql)?;
        let rows = statement.query_map([], language_from_row)?;
        Ok(rows.collect::<Result<_, _>>()?)
    }
}

fn language_from_row(row: &Row) -> rusqlite::Result<LanguagePreset> {
    Ok(LanguagePreset {
        code: row.get(0)?,
        name: row.get(1)?,
        word_characters: row.get(2)?,
        sentence_end_chars: row.get(3)?,
        sentence_exceptions: row.get(4)?,
        character_substitutions: row.get(5)?,
        rtl: row.get(6)?,
        remove_spaces: row.get(7)?,
        split_each_char: row.get(8)?,
        tts_locale: row.get(9)?,
        dictionary_url: row.get(10)?,
        translator_url: row.get(11)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::tests_support::test_database;
    use crate::presets::parse_preset;

    fn preset(code: &str, name: &str) -> LanguagePreset {
        parse_preset(&format!(
            r#"
            name = "{name}"
            code = "{code}"
            word_characters = "a-z"
            sentence_end_chars = ".!?"
            sentence_exceptions = "Dr.|Mr."
            character_substitutions = "'=’"
            rtl = true
            remove_spaces = false
            split_each_char = true
            tts_locale = "xx-XX"
            dictionary_url = "https://example.org/{{term}}"
            translator_url = ""
            "#
        ))
        .expect("test preset parses")
    }

    #[test]
    fn stores_and_reads_back_every_field() {
        let db = test_database();
        let original = preset("xx", "Testish");
        db.upsert_language(&original).unwrap();
        assert_eq!(db.language("xx").unwrap(), Some(original));
    }

    #[test]
    fn upsert_replaces_the_settings_of_an_existing_code() {
        let db = test_database();
        db.upsert_language(&preset("xx", "Old name")).unwrap();
        db.upsert_language(&preset("xx", "New name")).unwrap();
        let languages = db.languages().unwrap();
        assert_eq!(languages.len(), 1);
        assert_eq!(languages[0].name, "New name");
    }

    #[test]
    fn adding_a_missing_language_keeps_an_existing_one() {
        let db = test_database();
        db.upsert_language(&preset("xx", "Edited by the user"))
            .unwrap();
        db.add_language_if_missing(&preset("xx", "Built-in"))
            .unwrap();
        db.add_language_if_missing(&preset("yy", "New")).unwrap();
        assert_eq!(
            db.language("xx").unwrap().unwrap().name,
            "Edited by the user"
        );
        assert_eq!(db.language("yy").unwrap().unwrap().name, "New");
    }

    #[test]
    fn unknown_code_is_none() {
        let db = test_database();
        assert_eq!(db.language("zz").unwrap(), None);
    }

    #[test]
    fn languages_come_back_sorted_by_code() {
        let db = test_database();
        db.upsert_language(&preset("fr", "French")).unwrap();
        db.upsert_language(&preset("de", "German")).unwrap();
        let codes: Vec<String> = db
            .languages()
            .unwrap()
            .into_iter()
            .map(|l| l.code)
            .collect();
        assert_eq!(codes, ["de", "fr"]);
    }
}
