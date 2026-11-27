//! The vocabulary check: a session the app drives one band of words at a
//! time, and the calls that apply or undo its result.

use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use allreads_core::vocab_check::{CheckWord, MarkChoice, VocabCheck};

use crate::app::App;
use crate::error::{poisoned, AllreadsError};

/// A word to ask about in the vocabulary check. Pass it back to `answer`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct VocabWord {
    pub word: String,
    /// Its frequency rank; `None` for a made-up word (the app need not care).
    pub rank: Option<u32>,
}

/// One way to apply the check: how many words it marks and how many of them
/// the reader is expected to know.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct VocabChoice {
    /// Dictionary forms marked (their other forms come with them).
    pub words: u32,
    /// The expected share of right marks, 0 to 1.
    pub expected_accuracy: f64,
}

/// What the check found.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct VocabResult {
    /// False when the reader said "yes" to too many made-up words.
    pub reliable: bool,
    /// Roughly how many common words the reader knows.
    pub estimated_known: u32,
    /// The safe choice first, then a larger one if there is one.
    pub choices: Vec<VocabChoice>,
}

/// A vocabulary check in progress.
#[derive(uniffi::Object)]
pub struct VocabCheckSession {
    language_code: String,
    check: Mutex<VocabCheck>,
    /// Each ranked dictionary form's word forms, for marking.
    forms: Vec<Vec<String>>,
}

#[uniffi::export]
impl VocabCheckSession {
    /// The next words to ask, in order; empty when the check is over.
    pub fn next_words(&self) -> Result<Vec<VocabWord>, AllreadsError> {
        let words = self.check.lock().map_err(|_| poisoned())?.next_words();
        Ok(words
            .into_iter()
            .map(|w| VocabWord {
                word: w.word,
                rank: w.rank.map(|r| u32::try_from(r).unwrap_or(u32::MAX)),
            })
            .collect())
    }

    /// Records whether the reader knows `word`.
    pub fn answer(&self, word: VocabWord, yes: bool) -> Result<(), AllreadsError> {
        let word = CheckWord {
            word: word.word,
            rank: word.rank.map(|r| r as usize),
        };
        self.check
            .lock()
            .map_err(|_| poisoned())?
            .answer(&word, yes);
        Ok(())
    }

    /// What the answers so far say.
    pub fn result(&self) -> Result<VocabResult, AllreadsError> {
        let result = self.check.lock().map_err(|_| poisoned())?.result();
        Ok(VocabResult {
            reliable: result.reliable,
            estimated_known: u32::try_from(result.estimated_known).unwrap_or(u32::MAX),
            choices: result
                .choices
                .iter()
                .map(|c| VocabChoice {
                    words: u32::try_from(c.words).unwrap_or(u32::MAX),
                    expected_accuracy: c.expected_accuracy,
                })
                .collect(),
        })
    }
}

#[uniffi::export]
impl App {
    /// Whether the language's dictionary has word frequencies, so the check can run.
    ///
    /// # Errors
    /// Returns `Database` if the dictionary cannot be read.
    pub fn vocabulary_check_available(&self, language_code: String) -> Result<bool, AllreadsError> {
        Ok(self
            .with_dictionary(&language_code, |d| d.frequency_source())?
            .flatten()
            .is_some())
    }

    /// Starts a vocabulary check in the language.
    ///
    /// # Errors
    /// Returns `Missing` when the language's dictionary is not installed or has
    /// no word frequencies.
    pub fn start_vocabulary_check(
        &self,
        language_code: String,
    ) -> Result<Arc<VocabCheckSession>, AllreadsError> {
        let missing = || AllreadsError::Missing {
            what: format!("word frequencies for {language_code}"),
        };
        // A new seed per check, so a retake asks other words.
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let started = self.with_dictionary(&language_code, |dictionary| {
            let ranked = dictionary.ranked_words()?;
            let forms = dictionary.ranked_forms()?;
            // A string is a real word if the dictionary has it as a headword or a form.
            let is_word = |w: &str| {
                dictionary.word_kinds(w).map_or(true, |k| {
                    !k.parts_of_speech.is_empty() || !k.lemmas.is_empty()
                })
            };
            let check = VocabCheck::new(ranked, &is_word, seed);
            Ok((check, forms))
        })?;
        let (check, forms) = started.ok_or_else(missing)?;
        if forms.is_empty() {
            return Err(missing());
        }
        Ok(Arc::new(VocabCheckSession {
            language_code,
            check: Mutex::new(check),
            forms,
        }))
    }

    /// Marks the words of the session's choice number `choice` (0 is the safe
    /// one) known, with all their forms; words already saved are left as they
    /// are. Returns how many words were added.
    ///
    /// # Errors
    /// Returns `InvalidInput` for a choice the result does not offer.
    pub fn apply_vocabulary_check(
        &self,
        session: Arc<VocabCheckSession>,
        choice: u32,
    ) -> Result<u32, AllreadsError> {
        let keys = {
            let check = session.check.lock().map_err(|_| poisoned())?;
            let result = check.result();
            let chosen: MarkChoice = *result.choices.get(choice as usize).ok_or_else(|| {
                AllreadsError::InvalidInput {
                    reason: format!("no choice {choice}"),
                }
            })?;
            check.forms_to_mark(&chosen, &session.forms)
        };
        let added = self
            .db()?
            .mark_known_from_check(&session.language_code, &keys)?;
        Ok(u32::try_from(added).unwrap_or(u32::MAX))
    }

    /// Takes back the vocabulary check in the language: deletes the words it
    /// marked that are still untouched. Returns how many.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be written.
    pub fn undo_vocabulary_check(&self, language_code: String) -> Result<u32, AllreadsError> {
        let deleted = self.db()?.undo_vocabulary_check(&language_code)?;
        Ok(u32::try_from(deleted).unwrap_or(u32::MAX))
    }

    /// How many words a vocabulary check marked that are still as it left them.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn vocabulary_check_count(&self, language_code: String) -> Result<u32, AllreadsError> {
        let count = self.db()?.vocabulary_check_count(&language_code)?;
        Ok(u32::try_from(count).unwrap_or(u32::MAX))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in French word for `rank`: letters only, unique.
    fn word(rank: usize) -> String {
        let letters = ['b', 'd', 'f', 'l', 'm', 'n', 'p', 'r', 's', 't'];
        let mut n = rank;
        let mut word = String::from("mo");
        loop {
            word.push(letters[n % 10]);
            word.push('a');
            n /= 10;
            if n == 0 {
                return word;
            }
        }
    }

    /// Installs a dictionary of 3,000 ranked words; the most common one has
    /// the extra form "moba-s".
    fn install_dictionary(data: &std::path::Path) {
        let dir = data.join("dictionaries");
        std::fs::create_dir_all(&dir).unwrap();
        let mut conn = rusqlite::Connection::open(dir.join("fr-en.sqlite")).unwrap();
        let conn = conn.transaction().unwrap();
        conn.execute_batch(allreads_core::dictionary::SCHEMA)
            .unwrap();
        conn.execute_batch(allreads_core::dictionary::FREQUENCY_SCHEMA)
            .unwrap();
        conn.execute_batch(
            "INSERT INTO meta (key, value) VALUES ('format', '1'), ('pair', 'fr-en'), ('built', '2026-11-27'),
                 ('frequency_source', 'Test');",
        )
        .unwrap();
        for rank in 0..3_000_i64 {
            let key = word(rank as usize);
            conn.execute(
                "INSERT INTO entry (key, word, pos, ipa) VALUES (?1, ?1, 'noun', '')",
                [&key],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO frequency (rank, key, per_million) VALUES (?1, ?2, 1.0)",
                rusqlite::params![rank, key],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO frequency_form (rank, key) VALUES (?1, ?2)",
                rusqlite::params![rank, key],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO frequency_form (rank, key) VALUES (0, 'moba-s')",
            [],
        )
        .unwrap();
        conn.commit().unwrap();
    }

    #[test]
    fn a_check_runs_marks_words_with_their_forms_and_can_be_undone() {
        let dir = tempfile::tempdir().unwrap();
        let app = App::new(dir.path().display().to_string()).unwrap();
        assert!(!app.vocabulary_check_available("fr".into()).unwrap());
        assert!(matches!(
            app.start_vocabulary_check("fr".into()),
            Err(AllreadsError::Missing { .. })
        ));

        install_dictionary(dir.path());
        assert!(app.vocabulary_check_available("fr".into()).unwrap());
        let session = app.start_vocabulary_check("fr".into()).unwrap();
        // A reader who knows the 1,500 most common words and no made-up ones.
        let mut asked = 0;
        loop {
            let words = session.next_words().unwrap();
            if words.is_empty() {
                break;
            }
            for w in words {
                asked += 1;
                let yes = w.rank.is_some_and(|rank| rank < 1_500);
                session.answer(w, yes).unwrap();
            }
        }
        assert!(asked > 20, "{asked}");
        let result = session.result().unwrap();
        assert!(result.reliable);
        let safe = &result.choices[0];
        assert!(safe.words > 300 && safe.expected_accuracy > 0.9, "{safe:?}");

        let added = app.apply_vocabulary_check(session.clone(), 0).unwrap();
        // The marked words, plus the extra form of the most common one.
        assert_eq!(added, safe.words + 1);
        let saved = app.list_terms("fr".into()).unwrap();
        assert!(saved.iter().any(|t| t.term_key == "moba-s"));
        assert!(saved.iter().all(|t| t.imported_from == "Vocabulary check"));
        assert!(matches!(
            app.apply_vocabulary_check(session, 9),
            Err(AllreadsError::InvalidInput { .. })
        ));

        assert_eq!(app.vocabulary_check_count("fr".into()).unwrap(), added);
        assert_eq!(app.undo_vocabulary_check("fr".into()).unwrap(), added);
        assert!(app.list_terms("fr".into()).unwrap().is_empty());
    }
}
