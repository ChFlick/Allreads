-- Books read before `last_read_at` existed (V8) have no time, so they were
-- listed as never opened. A saved position past the start shows they were
-- read; give them a time before any real one, oldest import first, so they
-- count as "reading" and sort after books read since.
UPDATE book SET last_read_at = id
WHERE last_read_at IS NULL AND (chapter_index > 0 OR paragraph_index > 0);
