-- Counting saved words per language and status (the progress chart, after
-- every save) reads this index instead of every term of the language.
CREATE INDEX term_language_status ON term (language_code, status);
