//! Reading Anki's `.apkg` deck files for import: the notes, grouped by note
//! type, with each note's review progress.
//!
//! An `.apkg` is a zip holding the deck's collection, an SQLite database:
//! `collection.anki21b` (zstd-compressed, Anki 2.1.50 and later),
//! `collection.anki21` or `collection.anki2` (older). Newer files keep note
//! types in `notetypes`/`fields` tables; older ones as JSON in `col.models`.

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use serde::Deserialize;

use crate::import::{CardProgress, ImportSource, SourceNote};

/// Why a deck file could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ApkgError {
    #[error("not an Anki deck file: {0}")]
    Invalid(String),
    #[error("file error: {0}")]
    Io(#[from] io::Error),
    #[error("the deck's database could not be read: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

impl From<zip::result::ZipError> for ApkgError {
    fn from(error: zip::result::ZipError) -> Self {
        ApkgError::Invalid(error.to_string())
    }
}

/// The notes of an `.apkg` file, one [`ImportSource`] per note type. The
/// collection is unpacked into `work_dir` while reading and deleted after.
///
/// # Errors
/// Returns `Invalid` for a file that is not a deck, and `Io` or `Sqlite` if it
/// cannot be unpacked or read.
pub fn read_apkg(path: &Path, work_dir: &Path) -> Result<Vec<ImportSource>, ApkgError> {
    let mut archive = zip::ZipArchive::new(fs::File::open(path)?)?;
    // The newest collection the file holds; newer exports keep an old-style
    // placeholder next to it.
    let name = [
        "collection.anki21b",
        "collection.anki21",
        "collection.anki2",
    ]
    .into_iter()
    .find(|name| archive.file_names().any(|n| n.is_ok_and(|n| n == *name)))
    .ok_or_else(|| ApkgError::Invalid("it holds no Anki collection".into()))?;
    let mut entry = archive.by_name(name)?;
    let mut bytes = Vec::new();
    if name.ends_with('b') {
        // Newest format: the database is zstd-compressed.
        ruzstd::decoding::StreamingDecoder::new(&mut entry)
            .map_err(|e| ApkgError::Invalid(e.to_string()))?
            .read_to_end(&mut bytes)?;
    } else {
        entry.read_to_end(&mut bytes)?;
    }
    drop(entry);
    fs::create_dir_all(work_dir)?;
    let database = work_dir.join("import-collection.sqlite");
    fs::write(&database, &bytes)?;
    let result = read_collection(&database);
    let _ = fs::remove_file(&database);
    result
}

/// One note type: its name and field names in order.
struct NoteType {
    name: String,
    fields: Vec<String>,
}

/// The note types as older collections store them, JSON in `col.models`.
#[derive(Deserialize)]
struct LegacyModel {
    name: String,
    flds: Vec<LegacyField>,
}

#[derive(Deserialize)]
struct LegacyField {
    name: String,
    ord: i64,
}

fn read_collection(database: &Path) -> Result<Vec<ImportSource>, ApkgError> {
    let conn = Connection::open_with_flags(database, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let note_types = note_types(&conn)?;
    let progress = note_progress(&conn)?;
    let mut groups: HashMap<i64, Vec<SourceNote>> = HashMap::new();
    let mut statement = conn.prepare("SELECT id, mid, flds FROM notes ORDER BY id")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    for row in rows {
        let (id, note_type, fields) = row?;
        groups.entry(note_type).or_default().push(SourceNote {
            // Fields are separated by the "unit separator" character.
            fields: fields.split('\u{1f}').map(str::to_string).collect(),
            progress: Some(progress.get(&id).copied().unwrap_or(CardProgress::New)),
        });
    }
    let mut sources: Vec<ImportSource> = groups
        .into_iter()
        .map(|(id, notes)| {
            let note_type = note_types.get(&id);
            let width = notes.iter().map(|n| n.fields.len()).max().unwrap_or(0);
            ImportSource {
                name: note_type.map_or_else(|| format!("Note type {id}"), |t| t.name.clone()),
                fields: note_type
                    .map(|t| t.fields.clone())
                    .unwrap_or_else(|| (1..=width).map(|n| format!("Field {n}")).collect()),
                notes,
            }
        })
        .collect();
    // The note type with the most notes first: usually the one to import.
    sources.sort_by(|a, b| b.notes.len().cmp(&a.notes.len()).then(a.name.cmp(&b.name)));
    Ok(sources)
}

/// Note types by id, from the `notetypes` and `fields` tables (new
/// collections) or the JSON in `col.models` (old ones).
fn note_types(conn: &Connection) -> Result<HashMap<i64, NoteType>, ApkgError> {
    let has_table: bool = conn.query_row(
        "SELECT count(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'notetypes'",
        [],
        |row| row.get(0),
    )?;
    let mut types = HashMap::new();
    if has_table {
        let mut names = conn.prepare("SELECT id, name FROM notetypes")?;
        for row in names.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })? {
            let (id, name) = row?;
            types.insert(
                id,
                NoteType {
                    name,
                    fields: Vec::new(),
                },
            );
        }
        let mut fields = conn.prepare("SELECT ntid, name FROM fields ORDER BY ntid, ord")?;
        for row in fields.query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })? {
            let (id, name) = row?;
            if let Some(note_type) = types.get_mut(&id) {
                note_type.fields.push(name);
            }
        }
    } else {
        let models: Option<String> = conn
            .query_row("SELECT models FROM col", [], |row| row.get(0))
            .optional()?;
        let models: HashMap<String, LegacyModel> =
            serde_json::from_str(models.as_deref().unwrap_or("{}"))
                .map_err(|e| ApkgError::Invalid(format!("unreadable note types: {e}")))?;
        for (id, model) in models {
            let Ok(id) = id.parse() else { continue };
            let mut fields = model.flds;
            fields.sort_by_key(|f| f.ord);
            types.insert(
                id,
                NoteType {
                    name: model.name,
                    fields: fields.into_iter().map(|f| f.name).collect(),
                },
            );
        }
    }
    Ok(types)
}

/// Each note's progress, from its cards: suspended if all its cards are, else
/// the furthest of them. A card in review counts with its interval; new and
/// learning cards count as new.
fn note_progress(conn: &Connection) -> Result<HashMap<i64, CardProgress>, ApkgError> {
    // queue -1 = suspended, 2 = review; type 2 = a review card (even if buried).
    let mut statement = conn.prepare("SELECT nid, queue, type, ivl FROM cards")?;
    let rows = statement.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, i64>(2)?,
            row.get::<_, i64>(3)?,
        ))
    })?;
    let mut progress: HashMap<i64, CardProgress> = HashMap::new();
    for row in rows {
        let (note, queue, kind, interval) = row?;
        let card = if queue == -1 {
            CardProgress::Suspended
        } else if kind == 2 || queue == 2 {
            CardProgress::Interval(u32::try_from(interval.max(0)).unwrap_or(u32::MAX))
        } else {
            CardProgress::New
        };
        let best = match progress.get(&note) {
            None => card,
            Some(&current) => furthest(current, card),
        };
        progress.insert(note, best);
    }
    Ok(progress)
}

/// The further on of two cards of one note; a suspended card only counts if
/// all are suspended.
fn furthest(a: CardProgress, b: CardProgress) -> CardProgress {
    use CardProgress::{Interval, New, Suspended};
    match (a, b) {
        (Suspended, other) | (other, Suspended) => other,
        (Interval(x), Interval(y)) => Interval(x.max(y)),
        (Interval(x), New) | (New, Interval(x)) => Interval(x),
        (New, New) => New,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// Notes of a tiny French deck: three Basic notes and one "Vocab" note.
    const NOTES: &str = "INSERT INTO notes (id, mid, flds) VALUES
         (1, 10, 'le chat' || char(31) || 'cat'),
         (2, 10, 'chien' || char(31) || 'dog'),
         (3, 10, 'oiseau' || char(31) || 'bird'),
         (4, 20, 'maison' || char(31) || 'house' || char(31) || 'f.');
     INSERT INTO cards (nid, queue, type, ivl) VALUES
         (1, 2, 2, 30), (1, 0, 0, 0),
         (2, -1, 2, 5),
         (3, 1, 1, -600),
         (4, -1, 0, 0), (4, -1, 2, 40);";

    fn zip_with(dir: &Path, entries: &[(&str, Vec<u8>)]) -> std::path::PathBuf {
        let path = dir.join("deck.apkg");
        let mut writer = zip::ZipWriter::new(fs::File::create(&path).unwrap());
        for (name, bytes) in entries {
            writer
                .start_file(
                    *name,
                    zip::write::SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Stored),
                )
                .unwrap();
            writer.write_all(bytes).unwrap();
        }
        writer.finish().unwrap();
        path
    }

    fn check(sources: &[ImportSource]) {
        assert_eq!(sources.len(), 2);
        let basic = &sources[0];
        assert_eq!(
            (basic.name.as_str(), basic.fields.as_slice()),
            ("Basic", &["Front".to_string(), "Back".to_string()][..])
        );
        assert_eq!(basic.notes[0].fields, ["le chat", "cat"]);
        // The furthest card counts; all cards suspended means suspended.
        assert_eq!(basic.notes[0].progress, Some(CardProgress::Interval(30)));
        assert_eq!(basic.notes[1].progress, Some(CardProgress::Suspended));
        assert_eq!(basic.notes[2].progress, Some(CardProgress::New));
        let vocab = &sources[1];
        assert_eq!(vocab.fields, ["Word", "Meaning", "Gender"]);
        assert_eq!(vocab.notes[0].progress, Some(CardProgress::Suspended));
    }

    #[test]
    fn an_old_style_deck_is_read_with_note_types_from_json() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("c.anki2");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(&format!(
            "CREATE TABLE col (models TEXT);
             CREATE TABLE notes (id INTEGER, mid INTEGER, flds TEXT);
             CREATE TABLE cards (nid INTEGER, queue INTEGER, type INTEGER, ivl INTEGER);
             INSERT INTO col VALUES ('{{\"10\": {{\"name\": \"Basic\", \"flds\": [{{\"name\": \"Back\", \"ord\": 1}}, {{\"name\": \"Front\", \"ord\": 0}}]}},
                                        \"20\": {{\"name\": \"Vocab\", \"flds\": [{{\"name\": \"Word\", \"ord\": 0}}, {{\"name\": \"Meaning\", \"ord\": 1}}, {{\"name\": \"Gender\", \"ord\": 2}}]}}}}');
             {NOTES}"
        ))
        .unwrap();
        drop(conn);
        let apkg = zip_with(dir.path(), &[("collection.anki2", fs::read(&db).unwrap())]);
        check(&read_apkg(&apkg, &dir.path().join("work")).unwrap());
        // The unpacked copy is gone.
        assert!(!dir.path().join("work/import-collection.sqlite").exists());
    }

    #[test]
    fn a_new_style_compressed_deck_is_read() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("c.anki21b");
        let conn = Connection::open(&db).unwrap();
        conn.execute_batch(&format!(
            "CREATE TABLE notetypes (id INTEGER, name TEXT);
             CREATE TABLE fields (ntid INTEGER, ord INTEGER, name TEXT);
             CREATE TABLE notes (id INTEGER, mid INTEGER, flds TEXT);
             CREATE TABLE cards (nid INTEGER, queue INTEGER, type INTEGER, ivl INTEGER);
             INSERT INTO notetypes VALUES (10, 'Basic'), (20, 'Vocab');
             INSERT INTO fields VALUES (10, 1, 'Back'), (10, 0, 'Front'), (20, 0, 'Word'), (20, 1, 'Meaning'), (20, 2, 'Gender');
             {NOTES}"
        ))
        .unwrap();
        drop(conn);
        let compressed = ruzstd::encoding::compress_to_vec(
            fs::read(&db).unwrap().as_slice(),
            ruzstd::encoding::CompressionLevel::Fastest,
        );
        // Newer exports also hold an old-style placeholder collection.
        let apkg = zip_with(
            dir.path(),
            &[
                ("collection.anki2", b"placeholder".to_vec()),
                ("collection.anki21b", compressed),
            ],
        );
        check(&read_apkg(&apkg, &dir.path().join("work")).unwrap());
    }

    #[test]
    fn a_file_that_is_not_a_deck_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let not_zip = dir.path().join("x.apkg");
        fs::write(&not_zip, b"hello").unwrap();
        assert!(matches!(
            read_apkg(&not_zip, dir.path()),
            Err(ApkgError::Invalid(_))
        ));
        let empty = zip_with(dir.path(), &[("media", b"{}".to_vec())]);
        assert!(matches!(
            read_apkg(&empty, dir.path()),
            Err(ApkgError::Invalid(_))
        ));
    }
}
