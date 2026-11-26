-- For a book's difficulty: a word that is a form of another ("lisait" of
-- "lire") counts as known when its dictionary form is saved, and names
-- ("Gregor", "Paris") are left out. Books are counted again after this.
ALTER TABLE book_word ADD COLUMN lemma_key TEXT;
ALTER TABLE book_word ADD COLUMN is_name INTEGER NOT NULL DEFAULT 0;
UPDATE book SET words_counted_with = 0;
