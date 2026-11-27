-- Cards of forgotten words, waiting to be suspended (and tagged
-- "allreads-forgotten") in AnkiDroid. Kept by note id, because forgetting
-- deletes the word and its link; the word's key lets saving it again within
-- the Undo window cancel the suspension.
CREATE TABLE anki_suspend (
    note_id       INTEGER PRIMARY KEY,
    language_code TEXT NOT NULL,
    term_key      TEXT NOT NULL,
    queued_at     INTEGER NOT NULL
);
