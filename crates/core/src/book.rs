//! What every book format shares: its metadata and its errors.

use std::path::PathBuf;

use thiserror::Error;

/// Basic facts about a book, read from its metadata (or its file name for TXT).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BookMeta {
    pub title: Option<String>,
    pub author: Option<String>,
    pub language: Option<String>,
    /// Number of readable chapters in reading order.
    pub chapters: usize,
}

/// Errors from opening or reading a book file (EPUB or TXT).
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BookError {
    #[error("file not found: {}", .0.display())]
    NotFound(PathBuf),
    #[error("could not read the book: {0}")]
    Invalid(String),
    #[error("unsupported file type: {0}")]
    Unsupported(String),
    #[error("chapter {index} is out of range; the book has {count} chapters")]
    ChapterOutOfRange { index: usize, count: usize },
}
