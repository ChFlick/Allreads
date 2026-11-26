-- An offline dictionary file, built by `dictgen` and read by the app.
-- Bump FORMAT_VERSION in mod.rs when this changes.

-- About the file: format, language pair, source, build date.
CREATE TABLE meta (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

-- One headword and part of speech, e.g. "lit" (noun). `key` is the app's
-- term key for the word, so a tapped token looks it up directly. Entries that
-- only say "form of X" are not stored here; they are rows of `form`.
CREATE TABLE entry (
    id   INTEGER PRIMARY KEY,
    key  TEXT NOT NULL,
    word TEXT NOT NULL,
    pos  TEXT NOT NULL,
    ipa  TEXT NOT NULL
);

-- The entry's English meanings, in order.
CREATE TABLE sense (
    entry_id INTEGER NOT NULL REFERENCES entry (id),
    position INTEGER NOT NULL,
    gloss    TEXT NOT NULL,
    PRIMARY KEY (entry_id, position)
) WITHOUT ROWID;

-- Words in the dictionary's language with the same meaning, e.g. "lit" ->
-- "couche". Added after format 1 was published, without a format bump: the
-- table is optional, older files simply have none, and older apps ignore it.
CREATE TABLE synonym (
    entry_id INTEGER NOT NULL REFERENCES entry (id),
    position INTEGER NOT NULL,
    word     TEXT NOT NULL,
    PRIMARY KEY (entry_id, position)
) WITHOUT ROWID;

-- An inflected or alternative form and its dictionary form, e.g.
-- "lisait" -> "lire" (verb; "imperfect indicative singular third-person").
-- One row per form, lemma and part of speech; a form with several readings
-- has them in `tags`, separated by "; ". The primary key doubles as the
-- lookup index by `key`.
CREATE TABLE form (
    key   TEXT NOT NULL,
    lemma TEXT NOT NULL,
    pos   TEXT NOT NULL,
    tags  TEXT NOT NULL,
    PRIMARY KEY (key, lemma, pos)
) WITHOUT ROWID;
