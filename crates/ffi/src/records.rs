//! The plain records and enums that cross into Kotlin.

use allreads_core::db::term::{Status, Term};

use crate::error::AllreadsError;

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

/// A DeepL key's monthly usage, returned when the key works.
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Record)]
pub struct DeepLUsage {
    pub characters_used: u64,
    pub character_limit: u64,
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
