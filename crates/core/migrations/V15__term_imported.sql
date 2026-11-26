-- Where an imported word came from, shown as a badge ("Anki · French
-- vocabulary"), or '' for words saved in Allreads.
ALTER TABLE term ADD COLUMN imported_from TEXT NOT NULL DEFAULT '';
-- 1 when the word already has a card elsewhere (imported from Anki), so
-- Allreads never sends it to AnkiDroid and makes no duplicate card.
ALTER TABLE term ADD COLUMN card_elsewhere INTEGER NOT NULL DEFAULT 0;
