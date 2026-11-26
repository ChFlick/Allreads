-- When the book was last read, in milliseconds since 1970. NULL for a book
-- that was never opened. The library lists recently read books first.
ALTER TABLE book ADD COLUMN last_read_at INTEGER;
