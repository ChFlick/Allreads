use std::collections::HashMap;

use super::*;
use crate::word_tally::BookWord;

fn word(count: u32, lemma: Option<&str>, is_name: bool) -> BookWord {
    BookWord {
        count,
        lemma_key: lemma.map(str::to_string),
        is_name,
    }
}

fn book(id: i64, title: &str, author: &str, subjects: &[&str], popularity: u32) -> NewCatalogBook {
    NewCatalogBook {
        id,
        title: title.into(),
        author: author.into(),
        subjects: subjects.iter().map(|s| (*s).to_string()).collect(),
        url: format!("https://example.org/{id}.epub"),
        popularity,
        year: Some(1900 + i32::try_from(id).unwrap()),
    }
}

/// Two small French books: a tale and a novel with a name and a form.
fn sample_catalog(dir: &tempfile::TempDir) -> Catalog {
    let path = dir.path().join("fr.sqlite");
    let mut writer =
        CatalogWriter::create(&path, "fr", "1.3/fr-en 2026-11-25", "2026-12-01").unwrap();
    let tale: HashMap<String, BookWord> = [
        ("le", word(10, None, false)),
        ("chat", word(6, None, false)),
        ("dort", word(3, Some("dormir"), false)),
        ("souris", word(1, None, false)),
    ]
    .into_iter()
    .map(|(k, w)| (k.to_string(), w))
    .collect();
    writer
        .add_book(
            &book(
                1,
                "Contes du lundi",
                "Alphonse Daudet",
                &["Short stories"],
                50,
            ),
            &tale,
        )
        .unwrap();
    let novel: HashMap<String, BookWord> = [
        ("le", word(20, None, false)),
        ("dormait", word(5, Some("dormir"), false)),
        ("dormir", word(2, None, false)),
        ("misère", word(8, None, false)),
        ("valjean", word(30, None, true)),
    ]
    .into_iter()
    .map(|(k, w)| (k.to_string(), w))
    .collect();
    writer
        .add_book(
            &book(
                2,
                "Les Misérables",
                "Victor Hugo",
                &["Historical fiction", "Paris"],
                900,
            ),
            &novel,
        )
        .unwrap();
    writer.finish().unwrap();
    Catalog::open(&path).unwrap()
}

#[test]
fn a_catalog_lists_its_books_most_popular_first() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = sample_catalog(&dir);
    assert_eq!(catalog.language_code(), "fr");
    assert_eq!(catalog.count_stamp(), "1.3/fr-en 2026-11-25");
    assert_eq!(catalog.built(), "2026-12-01");
    let titles: Vec<&str> = catalog.books().iter().map(|b| b.title.as_str()).collect();
    assert_eq!(titles, ["Les Misérables", "Contes du lundi"]);
    let novel = &catalog.books()[0];
    assert_eq!(novel.subjects, ["Historical fiction", "Paris"]);
    assert_eq!(novel.year, Some(1902));
    // The name is left out of the running words.
    assert_eq!(novel.words, 35);
}

#[test]
fn search_ignores_case_and_accents_and_needs_every_word() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = sample_catalog(&dir);
    let ids = |query: &str| {
        catalog
            .search(query)
            .iter()
            .map(|b| b.id)
            .collect::<Vec<_>>()
    };
    assert_eq!(ids("miserables"), [2]);
    assert_eq!(ids("HUGO misérables"), [2]);
    assert_eq!(ids("short stories"), [1]);
    assert_eq!(ids("hugo daudet"), Vec::<i64>::new());
    assert_eq!(ids("  "), [2, 1]);
}

#[test]
fn the_fit_counts_saved_words_and_their_dictionary_forms() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = sample_catalog(&dir);
    let statuses: HashMap<String, Status> = [
        ("le".to_string(), Status::WellKnown),
        ("chat".to_string(), Status::Learning(2)),
        // Saving "dormir" makes "dort" and "dormait" known too.
        ("dormir".to_string(), Status::WellKnown),
    ]
    .into_iter()
    .collect();
    let fits = catalog.fit(&statuses).unwrap();
    let novel = fits.iter().find(|f| f.book_id == 2).unwrap();
    assert_eq!(
        novel.difficulty,
        Difficulty {
            words: 35,
            new_words: 8,
            learning_words: 0,
            // le, dormir (with dormait), misère
            unique_words: 3,
            unique_new_words: 1,
        }
    );
    // "misère" is new and occurs 8 times.
    assert_eq!(novel.worth_learning, 1);
    let tale = fits.iter().find(|f| f.book_id == 1).unwrap();
    assert_eq!(tale.difficulty.words, 20);
    assert_eq!(tale.difficulty.new_words, 1);
    assert_eq!(tale.difficulty.learning_words, 6);
    assert_eq!(tale.worth_learning, 0);
}

#[test]
fn the_most_frequent_new_words_come_by_dictionary_form() {
    let dir = tempfile::tempdir().unwrap();
    let catalog = sample_catalog(&dir);
    let statuses: HashMap<String, Status> = [("le".to_string(), Status::WellKnown)]
        .into_iter()
        .collect();
    // "dormait" (5) and "dormir" (2) count together as "dormir".
    assert_eq!(
        catalog.top_new_words(2, &statuses, 5).unwrap(),
        [("misère".to_string(), 8), ("dormir".to_string(), 7)]
    );
    assert_eq!(catalog.top_new_words(2, &statuses, 1).unwrap().len(), 1);
    assert!(catalog.top_new_words(99, &statuses, 5).is_err());
}

#[test]
fn a_book_added_twice_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let mut writer = CatalogWriter::create(&dir.path().join("fr.sqlite"), "fr", "x", "x").unwrap();
    writer
        .add_book(&book(1, "Un", "A", &[], 1), &HashMap::new())
        .unwrap();
    assert!(writer
        .add_book(&book(1, "Un", "A", &[], 1), &HashMap::new())
        .is_err());
}

#[test]
fn a_catalog_of_another_format_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("fr.sqlite");
    CatalogWriter::create(&path, "fr", "x", "x")
        .unwrap()
        .finish()
        .unwrap();
    Connection::open(&path)
        .unwrap()
        .execute("UPDATE info SET value = '99' WHERE key = 'format'", [])
        .unwrap();
    assert!(matches!(Catalog::open(&path), Err(CatalogError::Format(_))));
    let junk = dir.path().join("junk.sqlite");
    std::fs::write(&junk, b"not a database at all").unwrap();
    assert!(Catalog::open(&junk).is_err());
}

#[test]
fn saved_statuses_are_read_by_term_key() {
    let db = crate::db::tests_support::french_db();
    db.save_term(&crate::db::tests_support::new_term(
        "Chat",
        Status::Learning(2),
    ))
    .unwrap();
    let statuses = db.term_statuses("fr").unwrap();
    assert_eq!(statuses.get("chat"), Some(&Status::Learning(2)));
    assert!(db.term_statuses("de").unwrap().is_empty());
}
