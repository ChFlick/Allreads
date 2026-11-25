//! Builds a dictionary from a tiny hand-written extract and checks what lands in it.

use std::path::PathBuf;

use rusqlite::Connection;

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-fr.jsonl")
}

fn built() -> (tempfile::TempDir, Connection) {
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("fr-en.sqlite");
    let counts = allreads_dictgen::build(&fixture(), &output, "fr", "fr-en", "2026-11-25").unwrap();
    // lit (noun), lire (verb), l'homme; the form-of entries and the German one are not headwords.
    assert_eq!(counts.entries, 3);
    let conn = Connection::open(&output).unwrap();
    (dir, conn)
}

fn rows(conn: &Connection, sql: &str, key: &str) -> Vec<String> {
    let mut statement = conn.prepare(sql).unwrap();
    statement
        .query_map([key], |row| row.get::<_, String>(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

#[test]
fn headwords_keep_their_meanings_and_pronunciation() {
    let (_dir, conn) = built();
    let glosses = rows(
        &conn,
        "SELECT e.pos || ' ' || e.ipa || ' ' || s.gloss FROM entry e JOIN sense s ON s.entry_id = e.id
         WHERE e.key = ?1 ORDER BY e.id, s.position",
        "lit",
    );
    assert_eq!(glosses, ["noun /li/ bed", "noun /li/ layer"]);
    // A sub-sense keeps only its own, specific gloss, and repeats are dropped.
    assert_eq!(
        rows(
            &conn,
            "SELECT s.gloss FROM entry e JOIN sense s ON s.entry_id = e.id WHERE e.key = ?1",
            "lire"
        ),
        ["to read", "to read aloud"]
    );
}

#[test]
fn forms_point_to_their_lemma_once_with_their_tags() {
    let (_dir, conn) = built();
    let lit = rows(
        &conn,
        "SELECT lemma || ': ' || tags FROM form WHERE key = ?1",
        "lit",
    );
    // Listed both in lit's own entry and in lire's table: one row.
    assert_eq!(lit, ["lire: indicative present singular third-person"]);
    assert_eq!(
        rows(
            &conn,
            "SELECT lemma || ': ' || tags FROM form WHERE key = ?1",
            "lits"
        ),
        ["lit: plural"]
    );
    // Table placeholders, phrases and the lemma itself are not forms.
    for junk in ["fr-conj-auto", "lire", "avoir + past participle"] {
        assert!(
            rows(&conn, "SELECT lemma FROM form WHERE key = ?1", junk).is_empty(),
            "{junk}"
        );
    }
}

#[test]
fn keys_match_the_app_s_term_keys() {
    let (_dir, conn) = built();
    // Lower case, with the curly apostrophe made straight, as the reader's tokens are.
    assert_eq!(
        rows(&conn, "SELECT word FROM entry WHERE key = ?1", "l'homme"),
        ["L’Homme"]
    );
}

#[test]
fn the_file_says_what_it_is() {
    let (_dir, conn) = built();
    let meta = rows(
        &conn,
        "SELECT key || '=' || value FROM meta WHERE key != ?1 ORDER BY key",
        "source",
    );
    assert_eq!(
        meta,
        ["built=2026-11-25", "entries=3", "format=1", "pair=fr-en"]
    );
}

#[test]
fn a_broken_line_names_its_number() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("bad.jsonl");
    std::fs::write(
        &input,
        "{\"word\": \"a\", \"lang_code\": \"fr\"}\nnot json\n",
    )
    .unwrap();
    let error = allreads_dictgen::build(&input, &dir.path().join("out.sqlite"), "fr", "fr-en", "x")
        .unwrap_err();
    assert!(error.starts_with("line 2:"), "{error}");
}

#[test]
fn a_gzipped_extract_is_read_too() {
    use std::io::Write;
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("tiny.jsonl.gz");
    let mut gz = flate2::write::GzEncoder::new(
        std::fs::File::create(&input).unwrap(),
        flate2::Compression::fast(),
    );
    gz.write_all(&std::fs::read(fixture()).unwrap()).unwrap();
    gz.finish().unwrap();
    let counts =
        allreads_dictgen::build(&input, &dir.path().join("out.sqlite"), "fr", "fr-en", "x")
            .unwrap();
    assert_eq!(counts.entries, 3);
}
