-- Elision prefixes per language (see presets), and the form of a term as
-- first seen. `term.text` now holds the lower-case term key.
ALTER TABLE language ADD COLUMN elision_prefixes TEXT NOT NULL DEFAULT '';
ALTER TABLE term ADD COLUMN display_text TEXT NOT NULL DEFAULT '';
UPDATE term SET display_text = text;
