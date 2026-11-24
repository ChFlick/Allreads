//! XHTML to blocks: headings and paragraphs, with italic and bold spans kept.

use scraper::{node::Node, ElementRef, Html};

/// Elements that start and end a block.
const BLOCK_TAGS: &[&str] = &[
    "address",
    "article",
    "aside",
    "blockquote",
    "dd",
    "div",
    "dl",
    "dt",
    "figcaption",
    "figure",
    "footer",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "header",
    "hr",
    "li",
    "main",
    "ol",
    "p",
    "pre",
    "section",
    "table",
    "td",
    "th",
    "tr",
    "ul",
];

/// Elements whose contents are never reading text.
const SKIPPED_TAGS: &[&str] = &["head", "script", "style"];

const ITALIC_TAGS: &[&str] = &["i", "em", "cite", "dfn", "var"];
const BOLD_TAGS: &[&str] = &["b", "strong"];

/// What a block is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// A heading, level 1 to 6.
    Heading(u8),
    Paragraph,
}

/// Inline styling of a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Style {
    pub italic: bool,
    pub bold: bool,
}

/// A run of text with one style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub text: String,
    pub style: Style,
}

/// A heading or paragraph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub kind: BlockKind,
    pub spans: Vec<Span>,
}

impl Block {
    /// The block's plain text, all spans joined.
    #[must_use]
    pub fn text(&self) -> String {
        self.spans.iter().map(|span| span.text.as_str()).collect()
    }
}

/// Turns XHTML into blocks.
///
/// Text directly inside a container such as `<div>` or `<body>` also becomes
/// a block. Runs of ASCII whitespace collapse to one space, also across
/// spans; non-breaking spaces are kept, since French typography relies on
/// them. `<br>` becomes a space, and blank blocks are dropped.
#[must_use]
pub fn html_to_blocks(html: &str) -> Vec<Block> {
    let document = Html::parse_document(html);
    let mut builder = Builder {
        blocks: Vec::new(),
        pieces: Vec::new(),
        kind: BlockKind::Paragraph,
    };
    builder.walk(document.root_element(), Style::default());
    builder.flush();
    builder.blocks
}

struct Builder {
    blocks: Vec<Block>,
    /// Raw text of the block being built, with the style of each piece.
    pieces: Vec<(String, Style)>,
    kind: BlockKind,
}

impl Builder {
    fn walk(&mut self, node: ElementRef, style: Style) {
        for child in node.children() {
            match child.value() {
                Node::Text(text) => self.pieces.push((text.to_string(), style)),
                Node::Element(element) => {
                    let name = element.name();
                    if SKIPPED_TAGS.contains(&name) {
                        continue;
                    }
                    if name == "br" {
                        self.pieces.push((" ".into(), style));
                        continue;
                    }
                    let Some(child_element) = ElementRef::wrap(child) else {
                        continue;
                    };
                    if BLOCK_TAGS.contains(&name) {
                        let parent_kind = self.kind;
                        self.flush();
                        self.kind =
                            heading_level(name).map_or(BlockKind::Paragraph, BlockKind::Heading);
                        self.walk(child_element, style);
                        self.flush();
                        self.kind = parent_kind;
                    } else {
                        let child_style = Style {
                            italic: style.italic || ITALIC_TAGS.contains(&name),
                            bold: style.bold || BOLD_TAGS.contains(&name),
                        };
                        self.walk(child_element, child_style);
                    }
                }
                _ => {}
            }
        }
    }

    /// Turns the buffered pieces into a block, unless they are blank.
    fn flush(&mut self) {
        let mut spans: Vec<Span> = Vec::new();
        let mut pending_space: Option<Style> = None;
        let mut started = false;
        for (text, style) in self.pieces.drain(..) {
            for c in text.chars() {
                if c.is_ascii_whitespace() {
                    if started && pending_space.is_none() {
                        pending_space = Some(style);
                    }
                    continue;
                }
                if let Some(space_style) = pending_space.take() {
                    push_char(&mut spans, ' ', space_style);
                }
                push_char(&mut spans, c, style);
                started = true;
            }
        }
        let block = Block {
            kind: self.kind,
            spans,
        };
        if !block.text().trim().is_empty() {
            self.blocks.push(block);
        }
    }
}

fn push_char(spans: &mut Vec<Span>, c: char, style: Style) {
    match spans.last_mut() {
        Some(last) if last.style == style => last.text.push(c),
        _ => spans.push(Span {
            text: c.to_string(),
            style,
        }),
    }
}

fn heading_level(name: &str) -> Option<u8> {
    match name {
        "h1" => Some(1),
        "h2" => Some(2),
        "h3" => Some(3),
        "h4" => Some(4),
        "h5" => Some(5),
        "h6" => Some(6),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITALIC: Style = Style {
        italic: true,
        bold: false,
    };
    const BOLD: Style = Style {
        italic: false,
        bold: true,
    };
    const PLAIN: Style = Style {
        italic: false,
        bold: false,
    };

    fn span(text: &str, style: Style) -> Span {
        Span {
            text: text.into(),
            style,
        }
    }

    #[test]
    fn headings_keep_their_level() {
        let blocks = html_to_blocks("<h2>Titre</h2><p>Texte</p>");
        assert_eq!(blocks[0].kind, BlockKind::Heading(2));
        assert_eq!(blocks[1].kind, BlockKind::Paragraph);
    }

    #[test]
    fn italic_and_bold_become_spans() {
        let blocks = html_to_blocks("<p>Le <i>livre</i> était <b>très</b> long.</p>");
        assert_eq!(
            blocks[0].spans,
            vec![
                span("Le ", PLAIN),
                span("livre", ITALIC),
                span(" était ", PLAIN),
                span("très", BOLD),
                span(" long.", PLAIN),
            ]
        );
    }

    #[test]
    fn nested_styles_combine() {
        let blocks = html_to_blocks("<p><em>un <strong>deux</strong></em></p>");
        let both = Style {
            italic: true,
            bold: true,
        };
        assert_eq!(
            blocks[0].spans,
            vec![span("un ", ITALIC), span("deux", both)]
        );
    }

    #[test]
    fn whitespace_collapses_across_spans() {
        let blocks = html_to_blocks("<p>  un  <i> deux </i>  trois </p>");
        assert_eq!(blocks[0].text(), "un deux trois");
    }

    #[test]
    fn a_container_inside_a_heading_does_not_lose_the_heading_after_it() {
        let blocks = html_to_blocks("<div><h1>Titre</h1>Texte nu</div>");
        assert_eq!(blocks[0].kind, BlockKind::Heading(1));
        assert_eq!(blocks[1].kind, BlockKind::Paragraph);
        assert_eq!(blocks[1].text(), "Texte nu");
    }

    #[test]
    fn non_breaking_spaces_are_kept() {
        assert_eq!(
            html_to_blocks("<p>dit\u{a0}: oui</p>")[0].text(),
            "dit\u{a0}: oui"
        );
    }
}
