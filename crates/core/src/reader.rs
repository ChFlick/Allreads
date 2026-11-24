//! Chapters ready for the reader: blocks of tokens, parsed once and cached.
//!
//! Statuses are not part of a parsed chapter. They change whenever a word is
//! saved, so the caller joins them fresh from `Database::term_statuses`.

use serde::{Deserialize, Serialize};

use crate::text::blocks::{Block, BlockKind, Style};
use crate::text::tokens::tokenize;
use crate::text::LanguageRules;

/// One token as stored in the chapter cache.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedToken {
    pub text: String,
    /// The term key, for words only.
    pub key: Option<String>,
    /// Sentence index within the block.
    pub sentence: u32,
    pub italic: bool,
    pub bold: bool,
}

/// One heading or paragraph of tokens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedBlock {
    /// 1 to 6 for a heading, `None` for a paragraph.
    pub heading_level: Option<u8>,
    pub tokens: Vec<ParsedToken>,
}

/// A whole chapter, parsed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedChapter {
    pub blocks: Vec<ParsedBlock>,
}

impl ParsedChapter {
    /// The chapter as bytes for the cache.
    ///
    /// # Panics
    /// Never in practice: the chapter holds only strings, numbers and booleans,
    /// which always serialise.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("a parsed chapter always serialises")
    }

    /// Reads a chapter back from the cache. `None` if the bytes are not a
    /// chapter, in which case the caller parses it again.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        serde_json::from_slice(bytes).ok()
    }

    /// The number of word tokens, e.g. for reading statistics.
    #[must_use]
    pub fn word_count(&self) -> usize {
        self.blocks
            .iter()
            .flat_map(|block| &block.tokens)
            .filter(|token| token.key.is_some())
            .count()
    }
}

/// Tokenizes each block. A token takes the style of the span where it starts.
#[must_use]
pub fn parse_blocks(blocks: &[Block], rules: &LanguageRules) -> ParsedChapter {
    let blocks = blocks
        .iter()
        .map(|block| {
            let text = block.text();
            // Where each span ends (as a byte offset into the block text), and its style.
            let mut span_ends = Vec::with_capacity(block.spans.len());
            let mut end = 0;
            for span in &block.spans {
                end += span.text.len();
                span_ends.push((end, span.style));
            }
            let mut offset = 0;
            let tokens = tokenize(&text, rules)
                .into_iter()
                .map(|token| {
                    let style = style_at(&span_ends, offset);
                    offset += token.text.len();
                    ParsedToken {
                        text: token.text,
                        key: token.key,
                        sentence: u32::try_from(token.sentence).unwrap_or(u32::MAX),
                        italic: style.italic,
                        bold: style.bold,
                    }
                })
                .collect();
            ParsedBlock {
                heading_level: match block.kind {
                    BlockKind::Heading(level) => Some(level),
                    BlockKind::Paragraph => None,
                },
                tokens,
            }
        })
        .collect();
    ParsedChapter { blocks }
}

/// The style of the span that contains byte `offset`.
fn style_at(span_ends: &[(usize, Style)], offset: usize) -> Style {
    span_ends
        .iter()
        .find(|(end, _)| offset < *end)
        .map(|(_, style)| *style)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::blocks::html_to_blocks;
    use crate::text::test_rules::builtin;

    fn parse(html: &str) -> ParsedChapter {
        parse_blocks(&html_to_blocks(html), &builtin("fr"))
    }

    #[test]
    fn headings_and_paragraphs_keep_their_kind() {
        let chapter = parse("<h2>Chapitre un</h2><p>Il lisait.</p>");
        assert_eq!(chapter.blocks[0].heading_level, Some(2));
        assert_eq!(chapter.blocks[1].heading_level, None);
    }

    #[test]
    fn tokens_take_the_style_of_their_span() {
        let chapter = parse("<p>Le <i>livre</i> <b>long</b>.</p>");
        let styled: Vec<(&str, bool, bool)> = chapter.blocks[0]
            .tokens
            .iter()
            .map(|t| (t.text.as_str(), t.italic, t.bold))
            .collect();
        assert_eq!(
            styled,
            [
                ("Le", false, false),
                (" ", false, false),
                ("livre", true, false),
                (" ", false, false),
                ("long", false, true),
                (".", false, false),
            ]
        );
    }

    #[test]
    fn sentences_are_counted_within_each_block() {
        let chapter = parse("<p>Un. Deux.</p><p>Trois.</p>");
        let first: Vec<u32> = chapter.blocks[0]
            .tokens
            .iter()
            .map(|t| t.sentence)
            .collect();
        assert_eq!(first, [0, 0, 1, 1, 1]);
        assert!(chapter.blocks[1].tokens.iter().all(|t| t.sentence == 0));
    }

    #[test]
    fn cache_bytes_round_trip() {
        let chapter = parse("<h1>Titre</h1><p>L'homme <i>lit</i>.</p>");
        assert_eq!(
            ParsedChapter::from_bytes(&chapter.to_bytes()),
            Some(chapter)
        );
        assert_eq!(ParsedChapter::from_bytes(b"not json"), None);
    }

    #[test]
    fn word_count_counts_words_only() {
        assert_eq!(parse("<p>L'homme lit, vite.</p>").word_count(), 4);
    }
}
