-- How often each word occurs in a book, by term key, for its difficulty
-- ("12 % new words"). Filled the first time the difficulty is asked for.
CREATE TABLE book_word (
    book_id INTEGER NOT NULL REFERENCES book (id) ON DELETE CASCADE,
    key     TEXT NOT NULL,
    count   INTEGER NOT NULL,
    PRIMARY KEY (book_id, key)
) WITHOUT ROWID;

-- The parser version the book's words were counted with; 0 = not counted.
-- A book with no words at all has no `book_word` rows, so this tells
-- "counted, empty" from "not counted yet".
ALTER TABLE book ADD COLUMN words_counted_with INTEGER NOT NULL DEFAULT 0;

-- Saved words per language at the end of each day, for the progress chart.
-- Rewritten whenever words are saved, forgotten or marked known.
CREATE TABLE word_count_day (
    language_code TEXT NOT NULL REFERENCES language (code),
    day           TEXT NOT NULL,
    known         INTEGER NOT NULL,
    learning      INTEGER NOT NULL,
    PRIMARY KEY (language_code, day)
) WITHOUT ROWID;

-- Start the history with today's counts.
INSERT INTO word_count_day (language_code, day, known, learning)
SELECT language_code, date('now', 'localtime'),
       SUM(status = 99), SUM(status BETWEEN 1 AND 5)
FROM term GROUP BY language_code;
