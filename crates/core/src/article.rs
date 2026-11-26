//! Web articles: the readable text of a page (title, language, paragraphs),
//! without menus, footers and ads, so it can be imported like a TXT book.

use scraper::{ElementRef, Html, Selector};

use crate::text::normalize_whitespace;

/// What a web page holds for reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Article {
    pub title: String,
    /// The page's `lang`, e.g. `fr` or `fr-FR`, if it says.
    pub language: Option<String>,
    /// Headings and paragraphs in reading order.
    pub paragraphs: Vec<String>,
}

/// Elements whose text is never part of the article.
const SKIPPED: [&str; 10] = [
    "nav",
    "header",
    "footer",
    "aside",
    "form",
    "script",
    "style",
    "noscript",
    "figcaption",
    "button",
];

/// Elements that hold the article's text.
const TEXT_BLOCKS: &str = "p, h1, h2, h3, h4, blockquote, li";

fn selector(css: &str) -> Selector {
    // The selectors here are fixed strings, checked by the tests.
    Selector::parse(css).expect("valid selector")
}

/// True if `element` sits inside a menu, footer, script or similar.
fn is_skipped(element: ElementRef) -> bool {
    element
        .ancestors()
        .filter_map(ElementRef::wrap)
        .any(|ancestor| SKIPPED.contains(&ancestor.value().name()))
}

fn text_of(element: ElementRef) -> String {
    normalize_whitespace(&element.text().collect::<String>())
}

/// The total paragraph text directly inside `element`: a high score marks the
/// article's body, as opposed to a sidebar of short links.
fn paragraph_score(element: ElementRef, paragraphs: &Selector) -> usize {
    element
        .select(paragraphs)
        .filter(|p| !is_skipped(*p))
        .map(|p| text_of(p).len())
        .sum()
}

/// Finds the article in a page's HTML.
///
/// The body is the `<article>` or `<main>` element with the most paragraph
/// text; without one, the element whose direct `<p>` children hold the most
/// text. Text inside menus, headers, footers, sidebars and forms is left out,
/// and so are list items shorter than four words (usually links).
#[must_use]
pub fn extract(html: &str) -> Article {
    let page = Html::parse_document(html);
    let paragraphs = selector("p");

    let title = page
        .select(&selector(r#"meta[property="og:title"]"#))
        .find_map(|meta| meta.value().attr("content"))
        .map(normalize_whitespace)
        .filter(|t| !t.is_empty())
        .or_else(|| page.select(&selector("title")).next().map(text_of))
        .filter(|t| !t.is_empty())
        .or_else(|| page.select(&selector("h1")).next().map(text_of))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| "Web article".to_string());

    let language = page
        .select(&selector("html"))
        .next()
        .and_then(|html| html.value().attr("lang"))
        .map(|lang| lang.trim().to_string())
        .filter(|lang| !lang.is_empty());

    let containers = selector("article, main");
    let body = page
        .select(&containers)
        .max_by_key(|element| paragraph_score(*element, &paragraphs))
        .filter(|element| paragraph_score(*element, &paragraphs) > 0)
        .or_else(|| {
            // No <article>: the parent of the most paragraph text.
            page.select(&selector("body *"))
                .filter(|element| !is_skipped(*element))
                .max_by_key(|element| {
                    element
                        .children()
                        .filter_map(ElementRef::wrap)
                        .filter(|child| child.value().name() == "p")
                        .map(|p| text_of(p).len())
                        .sum::<usize>()
                })
        });

    let mut texts: Vec<String> = Vec::new();
    if let Some(body) = body {
        for block in body.select(&selector(TEXT_BLOCKS)) {
            if is_skipped(block) {
                continue;
            }
            // A <p> inside a <li> or <blockquote> is reached through both; keep the innermost.
            if block.select(&selector(TEXT_BLOCKS)).next().is_some() {
                continue;
            }
            let text = text_of(block);
            let is_short_item = block.value().name() == "li" && text.split_whitespace().count() < 4;
            if text.is_empty() || is_short_item || texts.last() == Some(&text) {
                continue;
            }
            texts.push(text);
        }
    }
    // The title usually opens the body as a heading too; once is enough.
    if texts.first() == Some(&title) {
        texts.remove(0);
    }
    Article {
        title,
        language,
        paragraphs: texts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PAGE: &str = r#"<!doctype html>
<html lang="fr-FR">
<head>
  <title>Le Coq et la Perle | Le Journal</title>
  <meta property="og:title" content="Le Coq et la Perle">
  <script>var ads = "Achetez maintenant";</script>
</head>
<body>
  <header><nav><a href="/">Accueil</a> <a href="/monde">Monde</a></nav></header>
  <aside><p>Abonnez-vous à notre lettre pour ne rien manquer.</p></aside>
  <main>
    <article>
      <h1>Le Coq et la Perle</h1>
      <p>Un jour un Coq détourna
         une Perle, qu'il donna au beau premier lapidaire.</p>
      <h2>La morale</h2>
      <blockquote><p>« Je la crois fine », dit-il.</p></blockquote>
      <ul><li><a href="/a">Partager</a></li><li>Un ignorant hérita d'un manuscrit.</li></ul>
      <figure><img src="coq.jpg"><figcaption>Un coq, vers 1900.</figcaption></figure>
      <form><button>Commenter</button></form>
    </article>
  </main>
  <footer><p>© Le Journal, tous droits réservés.</p></footer>
</body>
</html>"#;

    #[test]
    fn the_article_body_is_kept_and_the_rest_left_out() {
        let article = extract(PAGE);
        assert_eq!(article.title, "Le Coq et la Perle");
        assert_eq!(article.language.as_deref(), Some("fr-FR"));
        assert_eq!(
            article.paragraphs,
            [
                "Un jour un Coq détourna une Perle, qu'il donna au beau premier lapidaire.",
                "La morale",
                "« Je la crois fine », dit-il.",
                "Un ignorant hérita d'un manuscrit.",
            ]
        );
    }

    #[test]
    fn without_an_article_element_the_densest_block_wins() {
        let article = extract(
            "<html><head><title>Notes</title></head><body>
               <div class=menu><p>Menu</p></div>
               <div class=content><p>Première phrase assez longue du texte.</p><p>Deuxième phrase.</p></div>
             </body></html>",
        );
        assert_eq!(article.title, "Notes");
        assert_eq!(article.language, None);
        assert_eq!(
            article.paragraphs,
            ["Première phrase assez longue du texte.", "Deuxième phrase."]
        );
    }

    #[test]
    fn an_empty_page_gives_an_empty_article() {
        let article = extract("<html><body></body></html>");
        assert_eq!(article.title, "Web article");
        assert!(article.paragraphs.is_empty());
    }
}
