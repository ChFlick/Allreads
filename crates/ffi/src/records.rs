//! The plain records and enums that cross into Kotlin.

use allreads_core::db::term::{Status, Term, TermContext};

use crate::error::AllreadsError;

/// How hard a book is for the reader: its words against the saved ones.
/// Counts are running words, except the `unique_` ones.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BookDifficulty {
    pub words: u64,
    pub new_words: u64,
    pub learning_words: u64,
    pub unique_words: u64,
    pub unique_new_words: u64,
    /// `new_words` as a share of `words`, 0 to 100.
    pub new_percent: f64,
}

impl From<allreads_core::db::progress::Difficulty> for BookDifficulty {
    fn from(d: allreads_core::db::progress::Difficulty) -> Self {
        BookDifficulty {
            words: d.words,
            new_words: d.new_words,
            learning_words: d.learning_words,
            unique_words: d.unique_words,
            unique_new_words: d.unique_new_words,
            new_percent: d.new_percent(),
        }
    }
}

/// A book's difficulty, with the book it belongs to.
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct BookDifficultyOf {
    pub book_id: i64,
    pub difficulty: BookDifficulty,
}

/// Saved words in a language at the end of a day, for the progress chart.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DayCount {
    /// `YYYY-MM-DD`.
    pub day: String,
    pub known: u32,
    pub learning: u32,
}

/// A language's saved-word counts for every day of the last 90, for the chart.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct WordProgress {
    /// One per day up to today, oldest first; empty before any word is saved.
    pub days: Vec<DayCount>,
    pub known_this_week: u32,
}

impl From<allreads_core::db::progress::ProgressSeries> for WordProgress {
    fn from(series: allreads_core::db::progress::ProgressSeries) -> Self {
        WordProgress {
            days: series.days.into_iter().map(DayCount::from).collect(),
            known_this_week: series.known_this_week,
        }
    }
}

impl From<allreads_core::db::progress::DayCount> for DayCount {
    fn from(d: allreads_core::db::progress::DayCount) -> Self {
        DayCount {
            day: d.day,
            known: d.known,
            learning: d.learning,
        }
    }
}

/// Text written as a TXT book in app storage, ready for `import_book` once
/// the user has picked its language.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PreparedText {
    pub path: String,
    /// The book's title (its file name), e.g. the article's headline.
    pub title: String,
    /// The language the source says it is in, e.g. a web page's `fr-FR`.
    pub language: Option<String>,
}

/// Facts read from a book file before it is imported, e.g. to suggest its language.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BookMeta {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    pub chapters: u32,
}

impl From<allreads_core::book::BookMeta> for BookMeta {
    fn from(meta: allreads_core::book::BookMeta) -> Self {
        BookMeta {
            title: meta.title,
            author: meta.author,
            language: meta.language,
            chapters: saturate_u32(meta.chapters),
        }
    }
}

/// One book on the library screen.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct BookSummary {
    pub id: i64,
    pub title: String,
    pub author: Option<String>,
    pub language_code: String,
    pub cover_path: Option<String>,
    pub chapter_count: u32,
    /// Where the reader stopped.
    pub chapter_index: u32,
    pub paragraph_index: u32,
    /// When the book was last read, in milliseconds since 1970; `None` if never opened.
    pub last_read_at: Option<i64>,
}

/// The result of "mark the rest of this chapter as known": the ids of the
/// words it added (their number is the count), so the marking can be undone.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MarkedKnown {
    pub term_ids: Vec<i64>,
}

/// How well a word is known, for colouring it in the reader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum WordStatus {
    /// Not saved yet.
    New,
    /// Being learned, level 1 to 5.
    Learning {
        level: u8,
    },
    Ignored,
    WellKnown,
}

impl WordStatus {
    /// The stored status, or `None` for `New` (an unsaved word).
    pub(crate) fn to_status(self) -> Result<Option<Status>, AllreadsError> {
        match self {
            WordStatus::New => Ok(None),
            WordStatus::Learning { level } => {
                Status::learning(level)
                    .map(Some)
                    .ok_or_else(|| AllreadsError::InvalidInput {
                        reason: format!("learning level {level} is not 1 to 5"),
                    })
            }
            WordStatus::Ignored => Ok(Some(Status::Ignored)),
            WordStatus::WellKnown => Ok(Some(Status::WellKnown)),
        }
    }
}

/// A word to save from the word sheet.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct TermInput {
    pub language_code: String,
    /// The word or phrase as the reader saw it.
    pub text: String,
    /// Any status but `New`; use `forget_term` to un-save a word.
    pub status: WordStatus,
    pub translation: String,
    pub notes: String,
    pub lemma: Option<String>,
    /// The sentence the word was met in, for the Anki card. The four context
    /// fields are replaced together: give any of them and the ones left `None`
    /// become empty. Give none (as the Words screen does) to keep what is stored.
    pub sentence: Option<String>,
    /// That sentence's translation, if the reader already showed one.
    pub sentence_translation: Option<String>,
    pub source_book: Option<String>,
    pub source_chapter: Option<String>,
}

impl TermInput {
    /// The card context, or `None` if no part of it was given.
    pub(crate) fn context(&self) -> Option<TermContext> {
        if self.sentence.is_none()
            && self.sentence_translation.is_none()
            && self.source_book.is_none()
            && self.source_chapter.is_none()
        {
            return None;
        }
        let text = |value: &Option<String>| value.clone().unwrap_or_default();
        Some(TermContext {
            sentence: text(&self.sentence),
            sentence_translation: text(&self.sentence_translation),
            source_book: text(&self.source_book),
            source_chapter: text(&self.source_chapter),
        })
    }
}

/// A saved word, as the word sheet shows it.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct SavedTerm {
    pub id: i64,
    pub language_code: String,
    /// The term key, matching `Token::term_key`.
    pub term_key: String,
    /// The form as first seen.
    pub display_text: String,
    pub status: WordStatus,
    pub translation: String,
    pub notes: String,
    pub lemma: Option<String>,
}

impl From<Term> for SavedTerm {
    fn from(term: Term) -> Self {
        SavedTerm {
            id: term.id,
            language_code: term.language_code,
            term_key: term.text,
            display_text: term.display_text,
            status: term.status.into(),
            translation: term.translation,
            notes: term.notes,
            lemma: term.lemma,
        }
    }
}

impl From<Status> for WordStatus {
    fn from(status: Status) -> Self {
        match status {
            Status::Learning(level) => WordStatus::Learning { level },
            Status::Ignored => WordStatus::Ignored,
            Status::WellKnown => WordStatus::WellKnown,
        }
    }
}

/// A word, or the spaces and punctuation between words.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Token {
    pub text: String,
    /// The term key, for words only. Pass it back when saving the word.
    pub term_key: Option<String>,
    /// For words only.
    pub status: Option<WordStatus>,
    /// Sentence index within the block.
    pub sentence: u32,
    pub italic: bool,
    pub bold: bool,
    /// The saved phrase this token is part of, if any. Every token of the
    /// phrase, spaces and punctuation between its words too, carries it.
    pub phrase: Option<PhraseMark>,
    /// Taken for a name ("Gregor"): not marked as new, left out of the difficulty.
    pub name: bool,
    /// A form whose dictionary form is saved ("lisait" with "lire" saved):
    /// counted as known in the chapter's share of new words.
    pub form_of_saved: bool,
}

/// One occurrence of a saved phrase in a chapter.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PhraseMark {
    /// Unique within the chapter, so two occurrences side by side stay apart.
    pub id: u32,
    /// The phrase's term key, for looking it up or saving it again.
    pub term_key: String,
    pub status: WordStatus,
}

/// A phrase from the offline dictionary around a tapped word.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct PhraseSuggestion {
    /// Its first and last word, as indexes into the words passed in.
    pub first_word: u32,
    pub last_word: u32,
    /// As the dictionary writes it, e.g. "pierre précieuse".
    pub text: String,
    pub term_key: String,
    /// Its first meaning, e.g. "gemstone".
    pub gloss: String,
}

/// A heading or paragraph.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct ChapterBlock {
    /// 1 to 6 for a heading, `None` for a paragraph.
    pub heading_level: Option<u8>,
    pub tokens: Vec<Token>,
}

/// A chapter ready to render.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Chapter {
    pub book_id: i64,
    pub index: u32,
    pub blocks: Vec<ChapterBlock>,
}

/// What the app needs to know about a language.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct LanguageInfo {
    pub code: String,
    pub name: String,
    /// A web dictionary with `{term}` for the word, or empty for none.
    pub dictionary_url: String,
    /// The text-to-speech locale, e.g. `fr-FR`.
    pub tts_locale: String,
    pub right_to_left: bool,
}

/// A working DeepL key, with its monthly usage when DeepL shares it. A key
/// that may translate but not read usage (a scoped key) has no usage numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct DeepLKeyStatus {
    pub characters_used: Option<u64>,
    pub character_limit: Option<u64>,
}

/// A machine translation, and whether it came from the cache (free and instant).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct Translation {
    pub text: String,
    pub from_cache: bool,
}

/// Converts a count to `u32`, clamping instead of wrapping.
pub(crate) fn saturate_u32(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}

/// One card of an Anki note type.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AnkiCardTemplate {
    pub name: String,
    pub front: String,
    pub back: String,
}

/// The Anki note type for one language, to create in `AnkiDroid` once.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AnkiNoteType {
    pub name: String,
    pub fields: Vec<String>,
    pub cards: Vec<AnkiCardTemplate>,
    pub css: String,
}

impl From<allreads_core::anki::NoteType> for AnkiNoteType {
    fn from(note_type: allreads_core::anki::NoteType) -> Self {
        AnkiNoteType {
            name: note_type.name,
            fields: note_type.fields,
            cards: note_type
                .cards
                .into_iter()
                .map(|card| AnkiCardTemplate {
                    name: card.name,
                    front: card.front,
                    back: card.back,
                })
                .collect(),
            css: note_type.css,
        }
    }
}

/// A note waiting to go to `AnkiDroid`: add it when `note_id` is `None`,
/// otherwise update that note. Fields follow `AnkiNoteType::fields`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AnkiNote {
    pub term_id: i64,
    /// Pass back to `record_anki_push`.
    pub queue_id: i64,
    pub language_code: String,
    pub note_id: Option<i64>,
    pub fields: Vec<String>,
    pub tags: Vec<String>,
}

impl From<allreads_core::anki::AnkiNote> for AnkiNote {
    fn from(note: allreads_core::anki::AnkiNote) -> Self {
        AnkiNote {
            term_id: note.term_id,
            queue_id: note.queue_id,
            language_code: note.language_code,
            note_id: note.note_id,
            fields: note.fields,
            tags: note.tags,
        }
    }
}

/// A dictionary offered for download, from the published manifest.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct AvailableDictionary {
    /// e.g. `fr-en`.
    pub pair: String,
    /// The book language it is for, e.g. `fr`.
    pub language_code: String,
    pub url: String,
    pub built: String,
    pub download_bytes: u64,
    pub installed_bytes: u64,
    /// Pass back to `install_dictionary`.
    pub sha256: String,
}

impl From<allreads_core::dictionary::AvailableDictionary> for AvailableDictionary {
    fn from(d: allreads_core::dictionary::AvailableDictionary) -> Self {
        AvailableDictionary {
            pair: d.pair,
            language_code: d.language,
            url: d.url,
            built: d.built,
            download_bytes: d.download_bytes,
            installed_bytes: d.installed_bytes,
            sha256: d.sha256,
        }
    }
}

/// An installed dictionary, for Settings.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct InstalledDictionary {
    pub pair: String,
    pub built: String,
    pub bytes: u64,
}

impl From<allreads_core::dictionary::InstalledDictionary> for InstalledDictionary {
    fn from(d: allreads_core::dictionary::InstalledDictionary) -> Self {
        InstalledDictionary {
            pair: d.pair,
            built: d.built,
            bytes: d.bytes,
        }
    }
}

/// A headword: "lit" (noun), /li/, with its meanings.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DictionaryEntry {
    pub word: String,
    pub part_of_speech: String,
    pub ipa: String,
    pub senses: Vec<String>,
    /// Words in the book's language with the same meaning.
    pub synonyms: Vec<String>,
}

/// The word is a form of `lemma`, e.g. "lisait" of "lire".
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DictionaryForm {
    pub lemma: String,
    pub part_of_speech: String,
    /// Readings separated by "; ", e.g. "imperfect indicative singular third-person".
    pub tags: String,
}

/// What the offline dictionary says about a word.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DictionaryLookup {
    pub entries: Vec<DictionaryEntry>,
    pub forms: Vec<DictionaryForm>,
    /// The dictionary form to prefill the Lemma field with, if any.
    pub suggested_lemma: Option<String>,
    /// The form to send to a translator: "coq" for a capitalised "Coq" the
    /// dictionary knows as a common word, else the word as looked up.
    pub common_form: String,
}

impl DictionaryLookup {
    /// A lookup as the word drawer shows it, with the form to translate.
    pub(crate) fn new(lookup: allreads_core::dictionary::Lookup, common_form: String) -> Self {
        let suggested_lemma = lookup.suggested_lemma().map(str::to_string);
        DictionaryLookup {
            entries: lookup
                .entries
                .into_iter()
                .map(|e| DictionaryEntry {
                    word: e.word,
                    part_of_speech: e.pos,
                    ipa: e.ipa,
                    senses: e.senses,
                    synonyms: e.synonyms,
                })
                .collect(),
            forms: lookup
                .forms
                .into_iter()
                .map(|f| DictionaryForm {
                    lemma: f.lemma,
                    part_of_speech: f.pos,
                    tags: f.tags,
                })
                .collect(),
            suggested_lemma,
            common_form,
        }
    }
}
