-- Where a word was saved: the sentence, that sentence's translation, and the
-- book and chapter. They fill the Anki card's Sentence, Sentence translation
-- and Source fields. Empty for words saved without context.
ALTER TABLE term ADD COLUMN context_sentence TEXT NOT NULL DEFAULT '';
ALTER TABLE term ADD COLUMN context_translation TEXT NOT NULL DEFAULT '';
ALTER TABLE term ADD COLUMN source_book TEXT NOT NULL DEFAULT '';
ALTER TABLE term ADD COLUMN source_chapter TEXT NOT NULL DEFAULT '';

-- Small app settings that belong with the data, e.g. the AnkiDroid note type
-- and deck ids. Never API keys: those stay in the Android Keystore.
CREATE TABLE setting (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
