//! How long a 10,000-word chapter takes to open. Run with `cargo bench -p allreads-core`.
//! Step 11's target is under 300 ms on a mid-range phone; this measures the host.

use std::collections::HashMap;
use std::hint::black_box;

use allreads_core::db::term::Status;
use allreads_core::presets::builtin_presets;
use allreads_core::reader::{parse_blocks, ParsedChapter};
use allreads_core::text::blocks::html_to_blocks;
use allreads_core::text::LanguageRules;
use criterion::{criterion_group, criterion_main, Criterion};

const SENTENCES: &[&str] = &[
    "L’homme s'arrêta devant la <i>porte</i> et regarda longtemps la rue déserte.",
    "« Aujourd'hui, peut-être ? » dit-il, sans vraiment y croire.",
    "M. Dupont rit... puis il partit vers la gare, où l'attendait son frère.",
    "Il était trois heures, et la pluie tombait toujours sur les toits gris.",
];

/// A chapter of about `words` words in paragraphs of eight sentences.
fn chapter_html(words: usize) -> String {
    let mut html = String::from("<html><body><h1>Chapitre premier</h1>");
    let mut count = 0;
    while count < words {
        html.push_str("<p>");
        for sentence in SENTENCES.iter().cycle().take(8) {
            html.push_str(sentence);
            html.push(' ');
            count += sentence.split_whitespace().count();
        }
        html.push_str("</p>");
    }
    html.push_str("</body></html>");
    html
}

fn french_rules() -> LanguageRules {
    let french = builtin_presets()
        .unwrap()
        .into_iter()
        .find(|p| p.code == "fr")
        .unwrap();
    LanguageRules::new(&french).unwrap()
}

fn bench_open_chapter(c: &mut Criterion) {
    let html = chapter_html(10_000);
    let rules = french_rules();
    let parsed = parse_blocks(&html_to_blocks(&html), &rules);
    assert!(parsed.word_count() >= 10_000);
    let bytes = parsed.to_bytes();

    // 3,000 saved terms, a few of them in this chapter.
    let mut statuses: HashMap<String, Status> = (0..3_000)
        .map(|i| (format!("mot{i}"), Status::Learning(1)))
        .collect();
    for word in ["homme", "porte", "rue", "gare"] {
        statuses.insert(word.to_string(), Status::Learning(3));
    }

    c.bench_function("cold open: html to tokens (10k words)", |b| {
        b.iter(|| parse_blocks(&html_to_blocks(black_box(&html)), &rules));
    });
    c.bench_function("cached open: read cache bytes (10k words)", |b| {
        b.iter(|| ParsedChapter::from_bytes(black_box(&bytes)).unwrap());
    });
    c.bench_function("status join (10k words, 3k terms)", |b| {
        b.iter(|| {
            black_box(&parsed)
                .blocks
                .iter()
                .flat_map(|block| &block.tokens)
                .filter_map(|token| token.key.as_ref())
                .filter(|key| statuses.contains_key(key.as_str()))
                .count()
        });
    });
}

criterion_group!(benches, bench_open_chapter);
criterion_main!(benches);
