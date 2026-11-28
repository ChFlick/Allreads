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

/// The reader's answer to one word of the check.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct VocabAnswer {
    pub word: VocabWord,
    pub yes: bool,
}

/// What applying a check did.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct VocabApplied {
    /// Dictionary forms marked, as the chosen option said ("Mark 820 words").
    pub words: u32,
    /// Word forms saved as known, their other forms included, minus words
    /// that were saved already.
    pub forms_added: u32,
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

/// How a language's vocabulary checks held up while reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct VocabStats {
    /// Word forms the checks marked known.
    pub marked: u32,
    /// Of those, how many the reader set back since.
    pub corrected: u32,
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

    /// Records the answers to a band of words from `next_words`, in one call.
    pub fn answer_all(&self, answers: Vec<VocabAnswer>) -> Result<(), AllreadsError> {
        let mut check = self.check.lock().map_err(|_| poisoned())?;
        for answer in answers {
            let word = CheckWord {
                word: answer.word.word,
                rank: answer.word.rank.map(|r| r as usize),
            };
            check.answer(&word, answer.yes);
        }
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
        // Without made-up words nothing catches guessing: no check then.
        if forms.is_empty() || !check.can_catch_guessing() {
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
    /// are. Returns how many dictionary forms that was and how many word
    /// forms were added.
    ///
    /// # Errors
    /// Returns `InvalidInput` for a choice the result does not offer.
    pub fn apply_vocabulary_check(
        &self,
        session: Arc<VocabCheckSession>,
        choice: u32,
    ) -> Result<VocabApplied, AllreadsError> {
        let (words, keys) = {
            let check = session.check.lock().map_err(|_| poisoned())?;
            let result = check.result();
            let chosen: MarkChoice = *result.choices.get(choice as usize).ok_or_else(|| {
                AllreadsError::InvalidInput {
                    reason: format!("no choice {choice}"),
                }
            })?;
            (chosen.words, check.forms_to_mark(&chosen, &session.forms))
        };
        let added = self
            .db()?
            .mark_known_from_check(&session.language_code, &keys)?;
        Ok(VocabApplied {
            words: u32::try_from(words).unwrap_or(u32::MAX),
            forms_added: u32::try_from(added).unwrap_or(u32::MAX),
        })
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

    /// How the checks in the language held up while reading: word forms
    /// marked, and how many of them the reader set back since.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn vocabulary_check_stats(
        &self,
        language_code: String,
    ) -> Result<VocabStats, AllreadsError> {
        let stats = self.db()?.vocabulary_check_stats(&language_code)?;
        Ok(VocabStats {
            marked: u32::try_from(stats.marked).unwrap_or(u32::MAX),
            corrected: u32::try_from(stats.corrected).unwrap_or(u32::MAX),
        })
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

    /// 3,000 stand-in French words, most common first: random syllables, a
    /// sparse sample like a real vocabulary, so made-up words can be spliced
    /// from them.
    fn words() -> Vec<String> {
        let onsets = [
            "b", "d", "f", "g", "l", "m", "n", "p", "r", "s", "t", "v", "ch", "br", "tr", "pl",
        ];
        let vowels = ["a", "e", "i", "o", "u", "ou", "ai"];
        let codas = ["", "", "", "r", "n", "s", "l"];
        let mut state: u64 = 2026;
        let mut next = |n: usize| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ((state >> 33) % n as u64) as usize
        };
        let mut seen = std::collections::HashSet::new();
        let mut words = Vec::new();
        while words.len() < 3_000 {
            let syllables = 2 + next(3);
            let word: String = (0..syllables)
                .map(|_| {
                    format!(
                        "{}{}{}",
                        onsets[next(onsets.len())],
                        vowels[next(vowels.len())],
                        codas[next(codas.len())]
                    )
                })
                .collect();
            if seen.insert(word.clone()) {
                words.push(word);
            }
        }
        words
    }

    /// Installs a dictionary of 3,000 ranked words; the most common one has
    /// the extra form "extraform".
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
        for (rank, key) in words().into_iter().enumerate() {
            let rank = rank as i64;
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
            "INSERT INTO frequency_form (rank, key) VALUES (0, 'extraform')",
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
            // One call per band.
            let answers: Vec<VocabAnswer> = words
                .into_iter()
                .map(|word| {
                    asked += 1;
                    let yes = word.rank.is_some_and(|rank| rank < 1_500);
                    VocabAnswer { word, yes }
                })
                .collect();
            session.answer_all(answers).unwrap();
        }
        assert!(asked > 20, "{asked}");
        let result = session.result().unwrap();
        assert!(result.reliable);
        let safe = &result.choices[0];
        assert!(safe.words > 300 && safe.expected_accuracy > 0.9, "{safe:?}");

        let applied = app.apply_vocabulary_check(session.clone(), 0).unwrap();
        // The chosen words, as the option said; saved with the extra form of the most common one.
        assert_eq!(applied.words, safe.words);
        let added = applied.forms_added;
        assert_eq!(added, safe.words + 1);
        let saved = app.list_terms("fr".into()).unwrap();
        assert!(saved.iter().any(|t| t.term_key == "extraform"));
        assert!(saved.iter().all(|t| t.imported_from == "Vocabulary check"));
        assert!(matches!(
            app.apply_vocabulary_check(session, 9),
            Err(AllreadsError::InvalidInput { .. })
        ));

        assert_eq!(app.vocabulary_check_count("fr".into()).unwrap(), added);
        assert_eq!(
            app.vocabulary_check_stats("fr".into()).unwrap(),
            VocabStats {
                marked: added,
                corrected: 0
            }
        );
        assert_eq!(app.undo_vocabulary_check("fr".into()).unwrap(), added);
        assert!(app.list_terms("fr".into()).unwrap().is_empty());
    }

    #[test]
    fn a_check_that_cannot_make_up_words_does_not_start() {
        let dir = tempfile::tempdir().unwrap();
        let app = App::new(dir.path().display().to_string()).unwrap();
        // Only 20 short words: nothing to splice made-up words from.
        let path = dir.path().join("dictionaries");
        std::fs::create_dir_all(&path).unwrap();
        let conn = rusqlite::Connection::open(path.join("fr-en.sqlite")).unwrap();
        conn.execute_batch(allreads_core::dictionary::SCHEMA)
            .unwrap();
        conn.execute_batch(allreads_core::dictionary::FREQUENCY_SCHEMA)
            .unwrap();
        conn.execute_batch("INSERT INTO meta (key, value) VALUES ('format', '1'), ('pair', 'fr-en'), ('frequency_source', 'Test');")
            .unwrap();
        for rank in 0..20_i64 {
            let key = format!("mo{}", char::from(b'a' + rank as u8));
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
        drop(conn);
        assert!(matches!(
            app.start_vocabulary_check("fr".into()),
            Err(AllreadsError::Missing { .. })
        ));
    }
}
