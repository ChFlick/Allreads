-- The translation cache, keyed by text, both languages and provider. V1 keyed
-- only by sentence hash and provider, so a French-to-English and a
-- French-to-German translation of the same sentence would overwrite each other.
-- Nothing has shipped yet, so the old table is replaced. AI explanations
-- (step 22) get their own table, keyed by sentence, word and native language.
DROP TABLE sentence_cache;

CREATE TABLE translation_cache (
    source_text TEXT NOT NULL,
    source_lang TEXT NOT NULL,
    target_lang TEXT NOT NULL,
    -- Text that guided the translation, e.g. the sentence around a word. '' for none.
    context     TEXT NOT NULL DEFAULT '',
    provider    TEXT NOT NULL,
    translation TEXT NOT NULL,
    PRIMARY KEY (source_text, source_lang, target_lang, context, provider)
);
