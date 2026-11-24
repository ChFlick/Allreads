-- Parsed chapters (blocks and tokens, no statuses), so reopening a chapter
-- skips the parser. A row is used only while its parser_version matches the
-- code; statuses are always joined fresh.
CREATE TABLE chapter_cache (
    book_id        INTEGER NOT NULL REFERENCES book (id) ON DELETE CASCADE,
    chapter_index  INTEGER NOT NULL,
    parser_version INTEGER NOT NULL,
    data           BLOB NOT NULL,
    PRIMARY KEY (book_id, chapter_index)
);
