//! Looking words up in an installed dictionary.

use std::path::Path;

use rusqlite::{Connection, OpenFlags};

use super::{DictionaryError, FORMAT_VERSION};

/// An installed dictionary, opened read-only.
pub struct Dictionary {
    conn: Connection,
}

/// A headword: "lit" (noun), /li/, "bed; layer".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DictionaryEntry {
    pub word: String,
    pub pos: String,
    pub ipa: String,
    pub senses: Vec<String>,
}

/// The word is a form of `lemma`, e.g. "lisait" of "lire" (imperfect ...).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormOf {
    pub lemma: String,
    pub pos: String,
    /// Readings separated by "; ", e.g. "imperfect indicative singular third-person".
    pub tags: String,
}

/// A dictionary phrase found around a tapped word, e.g. "pierre précieuse".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhraseHit {
    /// Its first and last word, as indexes into the words passed in.
    pub first: usize,
    pub last: usize,
    /// The phrase as the dictionary writes it, and its term key.
    pub word: String,
    pub key: String,
    /// Its first meaning.
    pub gloss: String,
}

/// Everything the dictionary knows about one term key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Lookup {
    pub entries: Vec<DictionaryEntry>,
    pub forms: Vec<FormOf>,
}

impl Lookup {
    /// Nothing found.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty() && self.forms.is_empty()
    }

    /// The dictionary form to suggest for the Lemma field: the first lemma
    /// this word is a form of, if any.
    #[must_use]
    pub fn suggested_lemma(&self) -> Option<&str> {
        self.forms.first().map(|f| f.lemma.as_str())
    }
}

impl Dictionary {
    /// Opens an installed file read-only and checks its format.
    ///
    /// # Errors
    /// Returns `Invalid` for a file of another format, and `Sqlite` if it
    /// cannot be opened.
    pub fn open(path: &Path) -> Result<Self, DictionaryError> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        let dictionary = Dictionary { conn };
        let format = dictionary.meta("format")?;
        if format.as_deref() != Some(FORMAT_VERSION.to_string().as_str()) {
            return Err(DictionaryError::Invalid(format!(
                "format {} (expected {FORMAT_VERSION})",
                format.unwrap_or_else(|| "missing".into())
            )));
        }
        Ok(dictionary)
    }

    /// A value from the file's `meta` table, e.g. `pair` or `built`.
    ///
    /// # Errors
    /// Returns `Invalid` if the file has no `meta` table.
    pub fn meta(&self, key: &str) -> Result<Option<String>, DictionaryError> {
        use rusqlite::OptionalExtension;
        self.conn
            .query_row("SELECT value FROM meta WHERE key = ?1", [key], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|e| DictionaryError::Invalid(e.to_string()))
    }

    /// Reads every page of the file (SQLite's `quick_check`), to catch a
    /// damaged or truncated file before it is used.
    ///
    /// # Errors
    /// Returns `Invalid` if the check finds a problem.
    pub fn check(&self) -> Result<(), DictionaryError> {
        let result: String = self
            .conn
            .query_row("PRAGMA quick_check", [], |row| row.get(0))
            .map_err(|e| DictionaryError::Invalid(e.to_string()))?;
        if result == "ok" {
            Ok(())
        } else {
            Err(DictionaryError::Invalid(result))
        }
    }

    /// The dictionary's phrases among the words around word `index` (term
    /// keys of a sentence's words, in order), longest first.
    ///
    /// # Errors
    /// Returns `Sqlite` if a query fails.
    pub fn phrases_around(
        &self,
        keys: &[&str],
        index: usize,
    ) -> Result<Vec<PhraseHit>, DictionaryError> {
        use rusqlite::OptionalExtension;
        let mut statement = self.conn.prepare_cached(
            "SELECT e.word, (SELECT gloss FROM sense WHERE entry_id = e.id ORDER BY position LIMIT 1)
             FROM entry e WHERE e.key = ?1 ORDER BY e.id LIMIT 1",
        )?;
        let mut hits = Vec::new();
        for (first, last, key) in crate::text::phrases::runs_around(keys, index) {
            let found: Option<(String, Option<String>)> = statement
                .query_row([&key], |row| Ok((row.get(0)?, row.get(1)?)))
                .optional()?;
            if let Some((word, gloss)) = found {
                hits.push(PhraseHit {
                    first,
                    last,
                    word,
                    key,
                    gloss: gloss.unwrap_or_default(),
                });
            }
        }
        Ok(hits)
    }

    /// Entries and form links for a term key (as the reader's tokens carry).
    ///
    /// # Errors
    /// Returns `Sqlite` if a query fails.
    pub fn lookup(&self, key: &str) -> Result<Lookup, DictionaryError> {
        let mut entries: Vec<(i64, DictionaryEntry)> = Vec::new();
        {
            let mut statement = self.conn.prepare_cached(
                "SELECT id, word, pos, ipa FROM entry WHERE key = ?1 ORDER BY id",
            )?;
            let rows = statement.query_map([key], |row| {
                Ok((
                    row.get(0)?,
                    DictionaryEntry {
                        word: row.get(1)?,
                        pos: row.get(2)?,
                        ipa: row.get(3)?,
                        senses: Vec::new(),
                    },
                ))
            })?;
            for row in rows {
                entries.push(row?);
            }
        }
        let mut senses = self
            .conn
            .prepare_cached("SELECT gloss FROM sense WHERE entry_id = ?1 ORDER BY position")?;
        for (id, entry) in &mut entries {
            entry.senses = senses
                .query_map([*id], |row| row.get(0))?
                .collect::<Result<_, _>>()?;
        }
        let mut forms = self.conn.prepare_cached(
            "SELECT lemma, pos, tags FROM form WHERE key = ?1 ORDER BY lemma, pos",
        )?;
        let forms = forms
            .query_map([key], |row| {
                Ok(FormOf {
                    lemma: row.get(0)?,
                    pos: row.get(1)?,
                    tags: row.get(2)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(Lookup {
            entries: entries.into_iter().map(|(_, entry)| entry).collect(),
            forms,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dictionary::test_support::write_tiny;

    fn tiny() -> (tempfile::TempDir, Dictionary) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fr-en.sqlite");
        write_tiny(&path, "fr-en", FORMAT_VERSION);
        let dictionary = Dictionary::open(&path).unwrap();
        (dir, dictionary)
    }

    #[test]
    fn a_word_that_is_also_a_form_gives_both() {
        let (_dir, dictionary) = tiny();
        let lit = dictionary.lookup("lit").unwrap();
        assert_eq!(lit.entries.len(), 1);
        assert_eq!(lit.entries[0].senses, ["bed", "layer"]);
        assert_eq!(lit.entries[0].ipa, "/li/");
        assert_eq!(lit.suggested_lemma(), Some("lire"));
    }

    #[test]
    fn a_pure_form_points_to_its_lemma() {
        let (_dir, dictionary) = tiny();
        let lisait = dictionary.lookup("lisait").unwrap();
        assert!(lisait.entries.is_empty());
        assert_eq!(
            lisait.forms,
            [FormOf {
                lemma: "lire".into(),
                pos: "verb".into(),
                tags: "imperfect indicative singular third-person".into()
            }]
        );
    }

    #[test]
    fn phrases_around_a_word_are_found() {
        let (_dir, dictionary) = tiny();
        let hits = dictionary
            .phrases_around(&["une", "pierre", "précieuse", "brille"], 1)
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!((hits[0].first, hits[0].last), (1, 2));
        assert_eq!(hits[0].word, "pierre précieuse");
        assert_eq!(hits[0].gloss, "gemstone");
        assert!(dictionary.phrases_around(&["lit"], 0).unwrap().is_empty());
    }

    #[test]
    fn an_unknown_word_is_empty() {
        let (_dir, dictionary) = tiny();
        let nothing = dictionary.lookup("xyz").unwrap();
        assert!(nothing.is_empty());
        assert_eq!(nothing.suggested_lemma(), None);
    }

    #[test]
    fn a_file_of_another_format_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("old.sqlite");
        write_tiny(&path, "fr-en", FORMAT_VERSION + 1);
        assert!(matches!(
            Dictionary::open(&path),
            Err(DictionaryError::Invalid(_))
        ));
    }
}
