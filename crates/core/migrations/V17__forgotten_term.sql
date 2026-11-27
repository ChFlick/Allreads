-- A copy of each word forgotten in the last day, so Undo can bring it back
-- whole: its sentence and source, its import badge, its AnkiDroid card and
-- its place in the push queue. One row per word; older rows are cleared
-- whenever another word is forgotten.
CREATE TABLE forgotten_term (
    language_code       TEXT NOT NULL,
    text                TEXT NOT NULL,
    term_id             INTEGER NOT NULL,
    display_text        TEXT NOT NULL,
    lemma               TEXT,
    status              INTEGER NOT NULL,
    translation         TEXT NOT NULL,
    notes               TEXT NOT NULL,
    parent_id           INTEGER,
    context_sentence    TEXT NOT NULL,
    context_translation TEXT NOT NULL,
    source_book         TEXT NOT NULL,
    source_chapter      TEXT NOT NULL,
    imported_from       TEXT NOT NULL,
    card_elsewhere      INTEGER NOT NULL,
    -- The AnkiDroid note it was linked to, if any.
    note_id             INTEGER,
    deck_id             INTEGER,
    -- 1 when it was waiting to be pushed to AnkiDroid.
    was_queued          INTEGER NOT NULL,
    -- Unix milliseconds.
    forgotten_at        INTEGER NOT NULL,
    PRIMARY KEY (language_code, text)
);
