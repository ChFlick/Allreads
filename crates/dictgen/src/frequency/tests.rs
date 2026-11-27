use allreads_core::dictionary::{Dictionary, INDEXES, SCHEMA};

use super::*;

/// A small French dictionary: lit (bed), lire (to read), chien (dog), and the
/// forms "lit" and "lisait" of "lire".
fn dictionary(dir: &Path) -> std::path::PathBuf {
    let path = dir.join("fr-en.sqlite");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch(SCHEMA).unwrap();
    conn.execute_batch(
        "INSERT INTO entry (id, key, word, pos, ipa) VALUES
             (1, 'lit', 'lit', 'noun', ''), (2, 'lire', 'lire', 'verb', ''), (3, 'chien', 'chien', 'noun', '');
         INSERT INTO form (key, lemma, pos, tags) VALUES
             ('lit', 'lire', 'verb', ''), ('lisait', 'lire', 'verb', ''), ('chiens', 'chien', 'noun', '');
         INSERT INTO meta (key, value) VALUES ('format', '1'), ('pair', 'fr-en'), ('built', '2026-11-27');",
    )
    .unwrap();
    conn.execute_batch(INDEXES).unwrap();
    path
}

/// The columns this reader uses, between others as in the real file.
const LEXIQUE: &str =
    "ortho\tphon\tlemme\tcgram\tfreqlemfilms2\tfreqlemlivres\tfreqfilms2\tfreqlivres
lit\tli\tlit\tNOM\t30\t40\t30\t40
lit\tli\tlire\tVER\t300\t500\t20\t30
lisait\tlizE\tlire\tVER\t300\t500\t5\t15
lire\tliR\tlire\tVER\t300\t500\t40\t60
Chiens\tSj5\tchien\tNOM\t80\t60\t20\t10
chien\tSj5\tchien\tNOM\t80\t60\t60\t50
jean\tZA\tjean\tNOM\t9\t9\t9\t9
";

const FREQUENCY_WORDS: &str = "lire 500
lit 400
lisait 200
chiens 150
chien 140
john 900
12 80
";

fn build(
    source: &dyn FrequencySource,
    input: &str,
) -> (tempfile::TempDir, std::path::PathBuf, FrequencySummary) {
    let dir = tempfile::tempdir().unwrap();
    let path = dictionary(dir.path());
    let list = dir.path().join("list.txt");
    std::fs::write(&list, input).unwrap();
    let summary = add_frequencies(&path, source, &list, "fr").unwrap();
    (dir, path, summary)
}

#[test]
fn lexique_ranks_dictionary_forms_with_their_forms() {
    let (_dir, path, summary) = build(&Lexique, LEXIQUE);
    let dictionary = Dictionary::open(&path).unwrap();
    // lire: 25 + 10 + 50 = 85; chien: 15 + 55 = 70; lit (bed): 35. "jean" is not in the dictionary.
    assert_eq!(dictionary.ranked_words().unwrap(), ["lire", "chien", "lit"]);
    assert_eq!(
        dictionary.ranked_forms().unwrap(),
        [
            vec!["lire", "lisait", "lit"],
            vec!["chien", "chiens"],
            vec!["lit"]
        ]
    );
    assert_eq!(
        summary,
        FrequencySummary {
            ranked: 3,
            forms: 6,
            skipped: 1
        }
    );
    assert!(dictionary
        .frequency_source()
        .unwrap()
        .unwrap()
        .starts_with("Lexique 3"));
}

#[test]
fn frequency_words_take_dictionary_forms_from_the_dictionary_and_drop_names() {
    let (_dir, path, summary) = build(&FrequencyWords, FREQUENCY_WORDS);
    let dictionary = Dictionary::open(&path).unwrap();
    // lire: its own count, plus "lisait" (a form of lire); "lit" has an entry of
    // its own, so it counts as "lit". "john" and "12" are not words here.
    assert_eq!(dictionary.ranked_words().unwrap(), ["lire", "lit", "chien"]);
    assert_eq!(dictionary.ranked_forms().unwrap()[0], ["lire", "lisait"]);
    assert_eq!(summary.skipped, 2);
    assert!(dictionary
        .frequency_source()
        .unwrap()
        .unwrap()
        .starts_with("FrequencyWords"));
}

#[test]
fn adding_again_replaces_the_frequencies() {
    let (dir, path, _) = build(&FrequencyWords, FREQUENCY_WORDS);
    let list = dir.path().join("lexique.tsv");
    std::fs::write(&list, LEXIQUE).unwrap();
    add_frequencies(&path, &Lexique, &list, "fr").unwrap();
    let dictionary = Dictionary::open(&path).unwrap();
    assert_eq!(dictionary.ranked_words().unwrap(), ["lire", "chien", "lit"]);
    assert!(dictionary
        .frequency_source()
        .unwrap()
        .unwrap()
        .starts_with("Lexique"));
}

#[test]
fn a_dictionary_without_frequencies_has_none() {
    let dir = tempfile::tempdir().unwrap();
    let dictionary = Dictionary::open(&dictionary(dir.path())).unwrap();
    assert!(dictionary.ranked_words().unwrap().is_empty());
    assert!(dictionary.ranked_forms().unwrap().is_empty());
    assert_eq!(dictionary.frequency_source().unwrap(), None);
}

#[test]
fn sources_are_found_by_name_and_bad_files_are_errors() {
    assert!(source_named("Lexique").is_some());
    assert!(source_named("frequencywords").is_some());
    assert!(source_named("other").is_none());
    let dir = tempfile::tempdir().unwrap();
    let bad = dir.path().join("bad.tsv");
    std::fs::write(&bad, "word\tcount\nx\t1\n").unwrap();
    assert!(Lexique.read(&bad).unwrap_err().contains("ortho"));
    std::fs::write(&bad, "chat lots\n").unwrap();
    assert!(FrequencyWords.read(&bad).unwrap_err().contains("line 1"));
}
