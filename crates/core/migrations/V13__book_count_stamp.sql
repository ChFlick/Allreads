-- Which rules and which dictionary a book's words were counted with, e.g.
-- "1.3/fr-en 2026-11-26", or '' when not counted. A count whose stamp differs
-- from today's (new rules, a dictionary installed, updated or removed) is
-- done again, so nothing has to reset books when a dictionary changes.
ALTER TABLE book ADD COLUMN words_counted TEXT NOT NULL DEFAULT '';
ALTER TABLE book DROP COLUMN words_counted_with;
