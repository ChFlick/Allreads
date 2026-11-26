-- Articles that may open a word in an imported deck ("le chat"), stripped
-- when importing words. Filled from the presets on the next start.
ALTER TABLE language ADD COLUMN articles TEXT NOT NULL DEFAULT '';
