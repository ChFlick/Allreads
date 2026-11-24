-- Initial schema. Migrations only ever add; never edit a file that has shipped.

CREATE TABLE language (
    code                    TEXT PRIMARY KEY,
    name                    TEXT NOT NULL,
    word_characters         TEXT NOT NULL,
    sentence_end_chars      TEXT NOT NULL,
    sentence_exceptions     TEXT NOT NULL DEFAULT '',
    character_substitutions TEXT NOT NULL DEFAULT '',
    rtl                     INTEGER NOT NULL DEFAULT 0,
    remove_spaces           INTEGER NOT NULL DEFAULT 0,
    split_each_char         INTEGER NOT NULL DEFAULT 0,
    tts_locale              TEXT NOT NULL,
    dictionary_url          TEXT NOT NULL DEFAULT '',
    translator_url          TEXT NOT NULL DEFAULT ''
);

CREATE TABLE book (
    id              INTEGER PRIMARY KEY,
    title           TEXT NOT NULL,
    author          TEXT,
    language_code   TEXT NOT NULL REFERENCES language (code),
    file_path       TEXT NOT NULL,
    cover_path      TEXT,
    chapter_index   INTEGER NOT NULL DEFAULT 0,
    paragraph_index INTEGER NOT NULL DEFAULT 0,
    added_at        TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Status: 1-5 learning, 98 ignored, 99 well known. A word with no row is new.
CREATE TABLE term (
    id            INTEGER PRIMARY KEY,
    language_code TEXT NOT NULL REFERENCES language (code),
    text          TEXT NOT NULL,
    lemma         TEXT,
    status        INTEGER NOT NULL CHECK (status IN (1, 2, 3, 4, 5, 98, 99)),
    translation   TEXT NOT NULL DEFAULT '',
    notes         TEXT NOT NULL DEFAULT '',
    parent_id     INTEGER REFERENCES term (id) ON DELETE SET NULL,
    UNIQUE (language_code, text)
);

CREATE TABLE sentence_cache (
    sentence_hash TEXT NOT NULL,
    provider      TEXT NOT NULL,
    translation   TEXT,
    explanation   TEXT,
    PRIMARY KEY (sentence_hash, provider)
);

CREATE TABLE reading_stat (
    day        TEXT NOT NULL,
    book_id    INTEGER NOT NULL REFERENCES book (id) ON DELETE CASCADE,
    words_read INTEGER NOT NULL DEFAULT 0,
    new_terms  INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (day, book_id)
);

-- Links a term to the AnkiDroid note it was pushed as, so edits update it.
CREATE TABLE anki_link (
    term_id        INTEGER PRIMARY KEY REFERENCES term (id) ON DELETE CASCADE,
    note_id        INTEGER NOT NULL,
    deck_id        INTEGER NOT NULL,
    last_pushed_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Terms waiting to be pushed to AnkiDroid. One row per term at most.
CREATE TABLE anki_outbox (
    id         INTEGER PRIMARY KEY,
    term_id    INTEGER NOT NULL UNIQUE REFERENCES term (id) ON DELETE CASCADE,
    attempts   INTEGER NOT NULL DEFAULT 0,
    queued_at  TEXT NOT NULL DEFAULT (datetime('now'))
);
