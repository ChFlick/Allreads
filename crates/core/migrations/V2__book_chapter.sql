-- The table of contents: one row per readable chapter, in reading order.
-- A chapter with no heading has no title; the app shows "Chapter N" instead.
CREATE TABLE book_chapter (
    book_id       INTEGER NOT NULL REFERENCES book (id) ON DELETE CASCADE,
    chapter_index INTEGER NOT NULL,
    title         TEXT,
    PRIMARY KEY (book_id, chapter_index)
);
