//! Reading kaikki.org's Wiktionary extract: one JSON object per line, one
//! per word and part of speech. Only the fields we use are parsed; serde
//! ignores the rest.

use serde::Deserialize;

use allreads_core::text::LanguageRules;

/// One line of the extract.
#[derive(Debug, Deserialize)]
pub struct KaikkiEntry {
    pub word: String,
    #[serde(default)]
    pub pos: String,
    #[serde(default)]
    pub lang_code: String,
    #[serde(default)]
    pub senses: Vec<KaikkiSense>,
    #[serde(default)]
    pub sounds: Vec<KaikkiSound>,
    #[serde(default)]
    pub forms: Vec<KaikkiForm>,
}

#[derive(Debug, Deserialize)]
pub struct KaikkiSense {
    #[serde(default)]
    pub glosses: Vec<String>,
    /// For a form like "lisait": the word it is a form of ("lire").
    #[serde(default)]
    pub form_of: Vec<KaikkiWord>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct KaikkiWord {
    pub word: String,
}

#[derive(Debug, Deserialize)]
pub struct KaikkiSound {
    pub ipa: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct KaikkiForm {
    pub form: String,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// A headword with its meanings, ready to store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Headword {
    pub key: String,
    pub word: String,
    pub pos: String,
    pub ipa: String,
    pub glosses: Vec<String>,
}

/// A form and the dictionary form it belongs to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormLink {
    pub key: String,
    pub lemma: String,
    pub pos: String,
    /// Grammatical tags, sorted and space-separated, e.g. "imperfect indicative singular third-person".
    pub tags: String,
}

/// Tags that describe Wiktionary's tables rather than the form.
const NOISE_TAGS: [&str; 5] = [
    "form-of",
    "table-tags",
    "inflection-template",
    "multiword-construction",
    "class",
];

/// Turns one extract entry into a headword (if it has meanings of its own,
/// not only "form of X") and its form links: from "form of" senses (lisait → lire) and from the entry's own
/// table of forms (lire → lis, lisait, …).
#[must_use]
pub fn convert(entry: &KaikkiEntry, rules: &LanguageRules) -> (Option<Headword>, Vec<FormLink>) {
    // Keyed like the app keys saved terms, so phrases match: "bon, ben" is `bon ben`.
    let key = rules.term_key(&entry.word);
    if key.is_empty() {
        return (None, Vec::new());
    }
    let mut links = Vec::new();
    let mut glosses = Vec::new();
    // An entry whose every sense is "form of X" becomes form links only.
    let only_forms = entry.senses.iter().all(|s| !s.form_of.is_empty());
    for sense in &entry.senses {
        // Sub-senses repeat their parent first; the last gloss is the specific one.
        if let Some(gloss) = sense
            .glosses
            .last()
            .map(|g| g.trim())
            .filter(|g| !g.is_empty())
        {
            if !glosses.iter().any(|g: &String| g == gloss) {
                glosses.push(gloss.to_string());
            }
        }
        for lemma in &sense.form_of {
            if lemma.word != entry.word {
                links.push(FormLink {
                    key: key.clone(),
                    lemma: lemma.word.clone(),
                    pos: entry.pos.clone(),
                    tags: clean_tags(&sense.tags),
                });
            }
        }
    }
    for form in &entry.forms {
        let text = form.form.trim();
        let noise = form.tags.iter().any(|t| NOISE_TAGS.contains(&t.as_str()));
        // Tables also hold placeholders and phrases like "avoir + past participle".
        if noise || text == entry.word || text.contains('+') || text.contains(' ') {
            continue;
        }
        let form_key = rules.term_key(text);
        if !form_key.is_empty() {
            links.push(FormLink {
                key: form_key,
                lemma: entry.word.clone(),
                pos: entry.pos.clone(),
                tags: clean_tags(&form.tags),
            });
        }
    }
    let ipa = entry
        .sounds
        .iter()
        .find_map(|s| s.ipa.clone())
        .unwrap_or_default();
    let headword = (!glosses.is_empty() && !only_forms).then(|| Headword {
        key,
        word: entry.word.clone(),
        pos: entry.pos.clone(),
        ipa,
        glosses,
    });
    (headword, links)
}

/// Tags without the noise, sorted, and joined by spaces.
fn clean_tags(tags: &[String]) -> String {
    let mut kept: Vec<&str> = tags
        .iter()
        .map(String::as_str)
        .filter(|t| !NOISE_TAGS.contains(t))
        .collect();
    kept.sort_unstable();
    kept.dedup();
    kept.join(" ")
}
