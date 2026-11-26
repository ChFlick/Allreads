# Implementation plan

Nov 23, 2026 · Christoph Butz

## How to use this plan

There are 27 steps in 6 phases, and each step is sized for an evening or two. Each one ends with a **Done when** check, and most say which Rust concept it teaches.

- Do the steps in order; each one builds on the last. Phase 2 ends with the MVP and phase 3 with daily-use Anki.
- Write logic and tests in `crates/core` first, expose it through `crates/ffi`, then build the screen.
- Keep the FFI surface small: a few coarse calls returning plain records, never chatty per-word calls.
- Core code is synchronous; Kotlin calls it on `Dispatchers.IO`. Move to UniFFI async only if a step needs it.
- References: [The Rust Book](https://doc.rust-lang.org/book/), [Rustlings](https://github.com/rust-lang/rustlings), the [UniFFI user guide](https://mozilla.github.io/uniffi-rs/) and the [AnkiDroid API wiki](https://github.com/ankidroid/Anki-Android/wiki/AnkiDroid-API).

## Status and next work items (updated 2026-11-24)

Android Studio is not available yet, so the Rust side runs ahead of the plan order. Device and Gradle checks for steps 1, 2, 3, 5, 6 and 7 stay open and are batched for when the Android project exists. Step 8 is done; see `PROGRESS.md`.

The biggest open risk is that the M0 pipeline (Rust built for Android, called from Kotlin) is still unproven. The next items, in order:

1. **Android libraries in CI.** Add a job that runs `cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 build --release` with the NDK preinstalled on GitHub's Ubuntu runner (check that it is there on the first run). Bundled SQLite and zip compile C code, so this is the first real test of the cross-build.
   - **Done when** CI builds all three `.so` files. This is step 1's check, run in CI instead of locally.
   - *Done 2026-11-24:* the runner had NDK 30 preinstalled; the release libraries are about 7.4 MB (arm64) and 5.4 MB (armv7) before stripping.
2. **Step 9, Rust side.** See the additions under step 9 below.
   - **Done when** a Rust test imports the EPUB fixtures, reopens the database, and lists them with title, language and position.
   - *Done 2026-11-24:* see `library::tests` and the `App` tests in `crates/ffi`.
3. **Step 10, text pipeline.** See the additions under step 10. *Done 2026-11-24.*
4. **Step 11, `open_chapter`, Rust side.** Includes the chapter cache and the benchmark. The 300 ms check on a phone stays open. *Rust side done 2026-11-24.*
5. **Step 13, Rust side.** `App::save_term`, plus "mark rest of chapter as known" (F8) as one batch call. *Done 2026-11-24.*
6. **Step 14, translator.** Tested against a local mock HTTP server; real calls need a key. *Rust side done 2026-11-24, DeepL only.*

Android work to do in one batch once Android Studio is available: step 2's project, step 5's Gradle task and phone check, the Kotlin checks for steps 6 and 7, then the screens for steps 9, 12 and 13.

## v1 scope and order (settled 2026-11-25, after a grilling session)

1. **Phone smoke test** of the CI APK, and fixes. First run done 2026-11-25; its bugs are fixed (see `PROGRESS.md`).
2. **AnkiDroid** (steps 17a, 16, 17b, 18, 19), as planned in Phase 3. Step 20 (export) was dropped.
3. **Read-aloud** (step 25): a play button per sentence and a speak button in the word drawer. Done 2026-11-25.
4. **Offline French→English dictionary** (step 23), downloaded on demand in the app (decided 2026-11-25).
5. **Phrases** (step 24).

Kept as they are: untapped words stay new until marked (only "Mark the rest of this chapter as known"); five learning levels plus Known and Ignore; meanings in English (DeepL, dictionary and Anki agree); per-sentence translation only.

Dropped: parallel mode (the translation half of step 21; per-sentence buttons already cover it).

Deferred past v1: AI "Explain" (step 22), and all of step 26: the book browser, stats, and (since 2026-11-25) backup, which Christoph does not need; Anki keeps the cards. See "Roadmap after v1" below.

**What is left for v1 (2026-11-25):** step 19's phone check, then 26a (make the repository presentable) and 27 (release).

## Roadmap after v1

Ideas kept for later, roughly in order of interest. None is planned in detail.

- **Sentence cards** (planned 2026-11-26, first after v1; see the plan below): a word can have several cards, one per sentence and meaning.
- **Word families** (2026-11-26): treat the forms of a word as one word, so "lisait", "lis" and "lu" share the status of "lire" in the reader, can be marked known together, and do not each make their own card. Builds on the dictionary's form → lemma links (already used for the book difficulty) and on sentence cards (a form met in a new sentence adds a card to the family instead of a new word). Open questions: whether a form keeps its own status once saved on its own, how a family shows in Words, and what happens to forms already saved separately.

### Sentence cards (planned 2026-11-26)

**Why.** Allreads is used for sentence mining, where the unit is the sentence card, not the word. Today a word (one per spelling and language) has one translation, one example sentence and one Anki note. Meeting "lit" again in another sentence with another meaning ("bed" in *« Il dort dans son lit »*, "reads" in *« Il lit un livre »*) can only overwrite. Worse, re-saving a word at a new level replaces its sentence but keeps the old translation, so the Anki card can pair *« Il lit un livre »* with "bed".

**Model.** The word keeps what is about the word: status (level and colour in the reader), lemma, display text. A new **card** holds what is about one sentence: the sentence, the word's meaning in it, the sentence translation, the book and chapter, notes, and its own AnkiDroid note. A word has zero or more cards.

**Decisions (recommended defaults, to confirm before starting):**
1. The first save of a word in a sentence makes its first card automatically (mining stays one tap). Later sentences get a card only through "+ Card for this sentence".
2. "+ Card" also works for known words, for a meaning not known yet. The automatic first card still follows today's rule (levels 1–4).
3. Notes belong to the card (they usually explain that sentence).
4. Changing a word's level never changes its cards.

**Steps**, each with its check:
1. **Core: card table and migration.** New `card` table (term id, sentence, meaning, sentence translation, book, chapter, notes, created). A migration turns every saved word's current context (sentence, translation, sentence translation, source, notes) into its first card, keeping its Anki note link. `save_term` stops touching context; new `add_card`, `update_card`, `delete_card`, `cards_for_term`.
   - **Done when** core tests show: a migrated database has one card per word with context; re-saving a word at another level leaves its cards alone; two cards for one word are listed in order.
2. **Core: Anki per card.** The outbox and the note link move from the word to the card (`anki_outbox.card_id`, `anki_note.card_id`); `pending_anki_notes` builds one note per card (Word, Meaning = card meaning, Sentence, Sentence translation, Source, Notes). Forgetting a word removes its queued cards; sent notes stay in AnkiDroid as today.
   - **Done when** core tests show a word with two cards yields two pending notes, and recording a push for one leaves the other queued.
3. **FFI.** `SavedTerm` gains its cards (or a `cards(term_id)` call); `TermInput` loses the context fields and a `CardInput` is added. Small and coarse, as the FFI rules ask.
   - **Done when** the Kotlin bindings build and the FFI tests cover adding and listing cards.
4. **Android: AnkiSync per card.** Drain per card. The duplicate check (`findNote`) matches on the Sentence field instead of Word, so a second card for "lit" is not mistaken for the first.
   - **Done when** `AnkiSyncTest` covers two cards for one word becoming two notes, and a re-queued card updating its own note. **Phone check:** AnkiDroid marks notes with the same first field (Word) as duplicates; confirm both notes are added and how the duplicate marker looks.
5. **Android: drawer.** A saved word tapped in a new sentence shows its cards ("1 card: « Il dort dans son lit », bed") and "+ Card for this sentence" with DeepL's in-context meaning, editable with ✎. In a sentence that already has a card, the drawer shows that card's meaning and notes. Level chips change only the status.
   - **Done when** screenshots show both cases and, on the phone, adding a second card for a word sends a second note to AnkiDroid.
6. **Android: Words.** A word lists all its meanings ("lit: bed; reads") and its number of cards; the editor lists the cards, each editable and deletable.
   - **Done when** the editor can change and delete a card, and the change reaches AnkiDroid.

**Risks.** The migration touches every saved word and the Anki links; it needs a test on a copy of a real database before release. AnkiDroid's duplicate marker (step 4) may push the note type's first field towards something unique per card, which would mean a new note-type version.

- **Backup and restore** (F27, from step 26): the database, books and covers as one zip, saved through the system file picker or shared; restore on a new phone. Dropped from v1 on 2026-11-25.
- **Book browser** (from step 26): find and download books from Standard Ebooks and Project Gutenberg (OPDS catalogues) without leaving the app.
- **Stats** (F25, from step 26): known words per language, new words per day, % known per chapter.
- **AI "Explain"** (step 22): grammar and idioms explained by an OpenAI-compatible or Anthropic model with the user's own key.
- **More dictionaries:** German and Spanish → English through the same Dictionaries workflow.
- **Monolingual definitions:** real definitions in the book's language (e.g. French → French from the French Wiktionary, which kaikki.org also extracts). Needs a second reader in dictgen for that edition's format and a second download. Builds on the "meanings in the book's language" setting (2026-11-26), which today shows synonyms from the English Wiktionary.
- **Cloud voices and audio on cards** (from step 25): a second `Speech` implementation, and audio files saved into Anki cards.
- **Whole-chapter read-aloud** (dropped from step 25 in v1).
- **CSV export** (step 20 was dropped): only if someone asks.
- **iOS:** the Rust core with UniFFI's Swift bindings.


1. **Install the toolchain.** Install `rustup` (stable), Android Studio with SDK 35 and NDK r27, the Android targets, and `cargo-ndk`:
   - `rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`
   - `cargo install cargo-ndk`
   - Set `ANDROID_NDK_HOME`.
   - Start The Rust Book chapters 1–6 and the first Rustlings exercises alongside.
   - **Done when** `cargo ndk -t arm64-v8a build` succeeds on an empty library crate.
2. **Create the repo.** Set up the workspace layout from the plan (`crates/core`, `crates/ffi`, `android/`, `data/languages/`), plus the licence, README, `rustfmt.toml` and `clippy` settings.
   - Create the Android project in `android/` (Empty Compose Activity, minSdk 26).
   - **Done when** both `cargo build` and the Android project build locally.
3. **Set up CI.** Add GitHub Actions jobs: `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test`, and a Gradle `assembleDebug` that also builds the Rust library.
   - **Done when** CI is green on `main`.
4. **Port the language presets.** Convert Lukaisu's presets (split rules, word characters, RTL, TTS locale) into `data/languages/*.toml`, starting with French, German, Spanish and English. Load them in Rust with `serde` + `toml`.
   - *Learn:* structs, `derive`, `Result`.
   - **Done when** a unit test loads every preset.

## Phase 1 (M0): Rust ↔ Kotlin spike

5. **Hello from Rust.** In `crates/ffi`, add `uniffi::setup_scaffolding!()` and `#[uniffi::export] fn core_version() -> String`.
   - Add a Gradle task that runs `cargo ndk -t arm64-v8a -t armeabi-v7a -t x86_64 -o app/src/main/jniLibs build --release`, then `uniffi-bindgen generate --library <lib.so> --language kotlin`. Make `preBuild` depend on it, and add JNA (`net.java.dev.jna:jna:<version>@aar`).
   - Show the version string on screen.
   - **Done when** it runs on the emulator and on your phone.
6. **Records and errors across the boundary.** Add `#[derive(uniffi::Record)] struct BookMeta { title, author, language, chapters }` and a `#[derive(uniffi::Error)]` enum built with `thiserror`.
   - *Learn:* enums, `Result`, the `?` operator.
   - **Done when** an invalid path raises a typed Kotlin exception.
7. **EPUB spike.** Add `fn read_epub_meta(path) -> BookMeta` and `fn chapter_text(path, index) -> Vec<String>` using `rbook`, with XHTML stripped to paragraphs by `scraper`. Show them in a Compose `LazyColumn`.
   - Test with a French Gutenberg EPUB (for example *Le Comte de Monte-Cristo*).
   - **Done when** a full chapter scrolls smoothly on your phone. M0 is complete.

## Phase 2 (M1): reading MVP

8. **Database and the `App` object.** Add a schema for Language, Book, Term, SentenceCache, ReadingStat, AnkiLink and `anki_outbox`, with `refinery` migrations and repository functions. Expose `#[derive(uniffi::Object)] struct App { db: Mutex<Connection> }`, created once with the app's data directory.
   - *Learn:* ownership, `Arc` and `Mutex`, modules.
   - **Done when** repository tests pass against an in-memory database.
9. **Book import and library.** On the Kotlin side, the system file picker (`OpenDocument`) copies the file into app storage and calls `app.import_book(path, lang)`. Rust extracts the metadata, cover and table of contents and stores the book.
   - Build a library screen with covers, a language badge and progress.
   - **Done when** importing three EPUBs and one TXT lists them correctly after an app restart.
   - *Added 2026-11-24:*
     - Embed the built-in presets in the library (`include_str!`) and store them in the database when `App` opens. Android has no `data/languages/` folder at runtime, and `insert_book` needs the language row.
     - Add a migration for the table of contents (`book_chapter`: book, index, title). The V1 schema has no place for it.
     - Store the cover in the app's data folder and keep its path on the book.
     - `App` keeps opened books in memory (`rbook::Epub` is thread-safe), so chapter reads stop re-parsing the file. Move `read_epub_meta` and `chapter_text` onto `App` and remove the free functions.
     - TXT import wraps the whole file as one chapter, split on blank lines.
10. **Text pipeline.** Build `core::text`: XHTML becomes blocks (heading or paragraph, with italic and bold spans). Then split sentences and tokenize words with `unicode-segmentation` plus the preset rules (French elision "l'homme" → "l'" + "homme", hyphens, numbers).
    - Use Lukaisu's parser tests as cases, written as `insta` snapshots.
    - *Learn:* iterators, traits, `&str` versus `String`.
    - **Done when** the snapshots for French, German and English sample texts are stable.
    - *Added 2026-11-24:*
      - Replace `epub::html_to_paragraphs`, which returns plain strings, with blocks that keep their kind (heading or paragraph) and italic and bold spans. PRD F4 needs both.
      - Port Lukaisu's sentence-end rule (`findLatinSentenceEnd`) and use `parser-fixtures.json` (English, French and German cases) as golden tests.
      - Those fixtures have no elision, hyphen or decimal cases, so add our own: "l'homme" splits into "l'" + "homme", but "aujourd'hui" stays whole; "peut-être" is one word; "3,14" is not a word.
      - Term key (decided 2026-11-24): one shared `term_key` function. It applies Unicode NFC (with `unicode-normalization`), then the preset's character substitutions, then lower case, then trims and collapses inner whitespace to one space. NFC matters because EPUBs can store "é" as "e" plus a combining accent, which would otherwise not match.
      - Everything that matches words uses `term_key`: tokens in `open_chapter`, `save_term` and `find_term` (which today only trim), phrase matching (step 24), lemma links and dictionary lookups (steps 16 and 23), and vocabulary search (F17).
      - Keep the form as first seen in a `display_text` column (a migration) for the word sheet and the Anki card. German nouns lose meaning in lower case ("Haus" is a noun, "haus" is not a word).
      - Check in preset validation that each preset's word-character class compiles as a regex.
11. **`open_chapter`.** Return `Chapter { blocks: [Block { kind, tokens: [Token { text, is_word, term_key, status, sentence }] }] }`. Join tokens with terms through one query into a `HashMap`, and cache the parsed chapter as a blob.
    - Add a `criterion` benchmark.
    - **Done when** a 10,000-word chapter opens in under 300 ms on your phone.
    - *Added 2026-11-24:* the chapter cache needs a migration (`chapter_cache`: book, chapter index, parser version, blob). A cached chapter is reused only when its parser version matches, so tokenizer changes never show stale tokens. Statuses are always joined fresh, never cached.
12. **Reader screen.** Render each block in a `LazyColumn` as an `AnnotatedString`, with a background style per status. On tap, use `pointerInput` + `TextLayoutResult.getOffsetForPosition` to find the character offset, then the token. Save the scroll position as chapter + block.
    - **Done when** taps hit the right word and reopening the book resumes in place.
    - *Changed 2026-11-25 (Christoph's sketch):* the reader shows one sentence per row with a translate button on the right; the translation appears under the sentence. Tapping a word opens a bottom drawer that starts half open and pulls up for more context. The app has three tabs: Library (start screen), Words and Settings.
13. **Word sheet and saving.** A `ModalBottomSheet` shows the word, its sentence, status chips (1–5, known, ignore) and a translation field. `app.save_term(...)` saves it, then the chapter's statuses are refreshed without a full reload.
    - **Done when** saving a word recolours every occurrence in the chapter.
14. **Translator.** Add a `Translator` trait with a DeepL implementation (`ureq` with rustls; LibreTranslate dropped 2026-11-24), and cache results in SentenceCache. The key lives in the Android Keystore and is passed in per call.
    - *Learn:* trait objects, `Box<dyn Trait>`, HTTP and `serde_json`.
    - **Done when** tapping a new word shows a translation in under 1 s, and instantly the second time.
    - *Added 2026-11-24:* never hold the database lock during an HTTP call. Read the cache, release the lock, translate, then lock again to store the result. The key is never logged and never stored.
15. **Settings and polish.** Add language setup from presets, translator settings, font size, line spacing, dark theme, and "mark rest of chapter as known".
    - **Done when** you read a full chapter of a French novel on your phone and save words without friction. M1 is complete.
    - *Added 2026-11-24:* settings such as the translator choice and the Anki deck id need a `setting` table (a migration). API keys never go in it; they stay in the Android Keystore.

## Phase 3 (M2): AnkiDroid integration

*Planned in detail 2026-11-25.* Order: **17a** (Rust) first, since it can be fully built and tested without a phone, then 16, 17b, 18 and 19 in Kotlin. (Step 20, export, was dropped.)

**How a saved word becomes a card.** Saving a word with status 1–4 stores the term with its context and queues it in `anki_outbox`. Kotlin's `AnkiSync` drains the queue: Rust builds each note's fields, Kotlin calls `addNote` (or `updateNoteFields` when an `AnkiLink` exists), and Rust records the note id and removes the term from the queue. If AnkiDroid is missing or a call fails, the term stays queued and is retried on the next drain (app start, resume, permission granted, after each save). Statuses 5, Known and Ignore never create cards; forgetting a word leaves its card in Anki; editing a word that has a card updates that card.

**The AnkiDroid API (checked in AnkiDroid's source, 2026-11-25).** Library `com.ichi2.anki:api`, LGPL v3 (fine as a separate library in an MIT app). Calls used: `addNewCustomModel`, `addNewDeck`, `getModelList`, `getDeckList`, `addNote` (returns the note id), `updateNoteFields` (returns false if the note was deleted in Anki). Needs the runtime permission `com.ichi2.anki.permission.READ_WRITE_DATABASE` and a `<queries>` entry for `com.ichi2.anki`. Risk: version 2.0.0 is not on Maven Central; older versions come from JitPack (`com.github.ankidroid:Anki-Android:api-v1.1.0`), which CI can reach but the cloud session cannot. Fallback: call AnkiDroid's ContentProvider directly (the library is a thin wrapper). *Decided at step 16: the library from JitPack (`api-v1.1.0`), now that the session can reach it.*

**Choices (Christoph, 2026-11-25).**
- The deck is chosen in Settings: an existing AnkiDroid deck, or a new one (suggested name "Allreads").
- The card's sentence translation reuses the translation already shown while reading. A Settings option, off by default, fetches one from DeepL when saving a word without one.
- Tags: `allreads`, the language code, and a short book tag.
- Cards are **sentence mining** (Christoph, 2026-11-25): one card per saved word by default. Front: the sentence with the word in bold. Back: the word, translation, dictionary form, sentence translation and source. Recognition only (target language → English). A Settings option adds a second, word → meaning card.
- **Audio:** from the start, the card template speaks the front with Anki's built-in `{{tts …}}` tag, using the device's voice (no files). Audio files saved into the cards come with read-aloud (step 25), via `addMediaFromUri`.
- **One note type per language** (e.g. "Allreads (French)"), because the `{{tts}}` tag needs a fixed locale, taken from the preset's `tts_locale`. The note type's fields, templates and CSS are defined in Rust, so Kotlin only hands them to AnkiDroid.

17a. **Rust side (core first).**
    - Migration V7: three term columns (the sentence the word was saved in, that sentence's translation, and its source such as "Book title, chapter 3"), and a `setting` table (key, value) for the Anki note type and deck ids. API keys never go in it.
    - `save_term` takes the optional sentence, sentence translation and source, and queues the term when its status is 1–4 or it already has a card.
    - A note builder turns a term into the six fields (Word, Lemma, Translation, Sentence with the word in bold, Sentence translation, Source) and its tags. Text is HTML-escaped; the word is found by tokenizing the sentence and matching term keys, so case and apostrophe style don't matter. Unit and snapshot tests.
    - The note type per language: name, fields, the sentence card (and the optional word card), templates with the `{{tts}}` tag, and CSS.
    - FFI: `pending_anki_notes()`, `record_anki_push(term_id, note_id)`, `record_anki_failure(term_id)`, and getters and setters for the Anki ids and the deck.
    - **Done when** the Rust tests cover queuing rules, field building and escaping, and `cargo test` passes.

16. **Connect to AnkiDroid.** Add AnkiDroid's API library (`com.ichi2.anki.api`), the `com.ichi2.anki.permission.READ_WRITE_DATABASE` permission, and a `<queries>` entry for `com.ichi2.anki` (needed on Android 11+). Detect whether AnkiDroid is installed and request the runtime permission.
    - **Done when** the settings screen shows "Connected to AnkiDroid" or a clear install prompt.
    - *Planned 2026-11-25:* an `AnkiBridge` interface plus its AnkiDroid implementation (installed? permission granted? and the calls above), so sync logic can be tested on the JVM with a fake. Settings gets an AnkiDroid section: "Connected", "Grant access" or "Install AnkiDroid", and the deck picker.
17. **Note type and deck.** On first use, create the app's note type with `AddContentApi.addNewCustomModel`:
    - fields: Word, Lemma, Translation, Sentence (with the word in bold), Sentence translation, Source;
    - two card templates: word → meaning, and sentence → meaning.
    - Pick or create a deck, and store both ids in settings.
    - **Done when** the note type and deck appear in AnkiDroid.
    - *Planned 2026-11-25:* on first sync, create the "Allreads" note type with six fields and two card templates (word → meaning; sentence → meaning with the word in bold) and clean CSS. The deck comes from Settings. If the note type or deck was deleted in Anki, it is recreated.
18. **Push and update.** Rust builds an `AnkiNote` record (HTML fields, sentence with `<b>word</b>`, book and chapter as the source). Kotlin's `AnkiBridge` calls `addNote`, or `updateNoteFields` if an AnkiLink exists, and records the note id through `app.record_anki_link`.
    - Push on save for statuses 1–4; never create duplicates.
    - **Done when** editing a term's translation updates the same card.
    - *Planned 2026-11-25:* `AnkiSync.drain()` in Kotlin, written against `AnkiBridge`; a JVM test with a fake AnkiDroid covers add, update, a note deleted in Anki (re-added), and never creating duplicates. Before adding a note without a link, look the word up with `findDuplicateNotes` on the Word field and relink, so forgetting and re-saving a word does not make a second card. Pass each note's `queue_id` back to `record_anki_push`, so a word edited during a push stays queued.
19. **Outbox and retry.** When AnkiDroid isn't available, the push goes into `anki_outbox` in Rust. Drain the queue on app start and after permission is granted.
    - **Done when** words saved with AnkiDroid uninstalled arrive after it's reinstalled.
    - *Planned 2026-11-25:* drain on app start, on resume and after the permission is granted; count attempts; Settings shows "N words waiting for AnkiDroid".
20. ~~`.apkg` and CSV export~~ *Dropped 2026-11-25.* AnkiDroid syncs cards to desktop Anki through AnkiWeb, and the whole database is backed up at step 26. A CSV export can come back with backup if someone asks.
    - M2 is complete when step 19 passes: save 20 words while reading, sync AnkiDroid, and review them on desktop.

## Phases 4–6 (M3–M5): translation and AI, audio, library and release

21. *(Sentence translation is done by the per-row translate button, step 12; parallel mode was dropped 2026-11-25.)* **Sentence translation and parallel mode.** Add `translate_sentence` with caching. Parallel mode renders a muted translation under each sentence and pre-translates the visible blocks in the background.
    - **Done when** parallel mode stays smooth while scrolling a chapter.
    - *Added 2026-11-24:* the sentence cache keeps case, since case changes a translation, but normalises the text first (NFC, character substitutions, collapsed whitespace) so the same sentence always hits the cache. The key must also include the source and target language. V1 keys only by hash and provider, so a French-to-English and a French-to-German translation of the same sentence would collide. Fix this with a migration before step 14 writes to the cache.
22. *(Deferred past v1; see the roadmap.)* **AI "Explain".** Add an `AiProvider` trait with OpenAI-compatible (which covers many hosts, including local models) and Anthropic implementations. The prompt includes the target and native language, the sentence and the word, and asks for meaning in context, grammar and why this form. Answers are cached per sentence + word, and the sheet shows a cost estimate before the call.
    - **Done when** "Explain" answers in the sheet and repeats are free.
    - *Added 2026-11-24:* the cache key combines the normalised sentence, the word's `term_key` and the native language.
23. **Offline dictionaries.** A separate binary, `crates/dictgen`, filters kaikki.org Wiktionary JSONL into a compact SQLite file per language pair, published as a GitHub release asset. The app downloads it on demand and uses it for lookups and lemma linking (F12, F16).
    - *Learn:* streaming large files, CLI tools with `clap`.
    - **Done when** French → English lookups work in flight mode.
    - *Added 2026-11-24:* `dictgen` stores headwords and inflected forms by `term_key`, so lookups match tokens directly.
    - *Measured 2026-11-24 (French → English spike):*
      - The raw kaikki.org French extract is 593 MB (404,127 entries). Filtered to word, part of speech, English definitions, IPA and form → lemma links, it is **43 MB as SQLite and 14 MB gzipped**. That is 103,460 words, 127,613 definitions and 333,157 inflected forms.
      - Capping definitions at three per word saves almost nothing; the form links are most of the size, and they are what powers lemma suggestions.
      - Lookups take under 0.5 ms. Spot checks were good: "lit" gives both "third-person singular of lire" and "bed"; "allait" gives "imperfect of aller"; "aujourd'hui", "peut-être" and "l'" match our tokenizer's keys.
      - To fix in `dictgen`: keep the form tags so labels say the person and tense (some only say "inflection of manger:"), and drop duplicate form rows.
    - *Planned 2026-11-24, personal build:* superseded 2026-11-25, see below.
    - *Scoped with Christoph 2026-11-25: download on demand, no bundled flavour.*
      - **Scope:** French → English only in v1; `dictgen` stays generic, so other pairs are a workflow input later.
      - **Hosting:** a separate GitHub Actions workflow, run by hand (not on every push), downloads the kaikki.org extract, runs `dictgen`, and uploads `fr-en.sqlite.gz` plus a small `dictionaries.json` manifest (pair, version date, download and installed size, SHA-256) to a fixed `dictionaries` release.
      - **Settings → Dictionaries:** each pair with its sizes ("14 MB download, 43 MB on the phone"), Download with progress, Remove, and the installed version's date. On mobile data the app asks before downloading. No update checks: Remove and Download again gets the newest.
      - **Interrupted downloads** start over; a partial file is deleted and never used.
      - **The word drawer** shows definitions with part of speech, and the dictionary form ("lisait → lire, imperfect, 3rd person"), tappable to show that entry. The Lemma field is prefilled from it and stays editable. With no dictionary installed, a French word's drawer shows a small "Get the offline dictionary" link.
      - **Licence:** Wiktionary data is CC BY-SA. Credit Wiktionary and kaikki.org in Settings → About and next to the download; the file keeps its own licence.
      - **Split of work:** Kotlin downloads the `.gz` to a temporary file with progress (no callbacks across the FFI). Rust then installs it: checks the SHA-256, unpacks, checks the schema, and moves it into place, so a broken file is never opened. Rust opens it read-only for `lookup`, which takes the word's term key and returns entries, senses and form → lemma links. Removing deletes the file.
      - *Built 2026-11-25 (23a):* 43 MB installed and 10.8 MB to download, smaller than the spike, because the form table has no rowid and no separate index (its primary key starts with the term key), and each form's readings are merged into one row.
      - **Sub-steps:** 23a `dictgen` and its workflow (with the measured fixes: keep form tags, drop duplicate form rows); 23b core install, lookup and FFI with tests on a tiny fixture dictionary; 23c the Settings section, download and drawer.
24. **Phrase selection.** Long-press and drag across tokens to select a phrase, saved as a multi-word term. Phrases are matched in `open_chapter` (longest match first).
    - **Done when** a saved idiom is highlighted wherever it appears.
    - *Decided 2026-11-25:* the offline dictionary also suggests phrases. Tapping a word shows the dictionary's multi-word entries found around it in the sentence (e.g. "pierre précieuse: gemstone" when tapping "Pierre"), with "Save as phrase". Long-press and drag stays, for phrases the dictionary does not know. The French dictionary has about 17,500 multi-word entries.
    - *Added 2026-11-24:* a phrase's key is the `term_key` of the whole phrase. Matching compares the keys of consecutive word tokens, so punctuation and line breaks between words don't break a match.
25. **Read-aloud.** *Scoped with Christoph 2026-11-25.* Android `TextToSpeech`, all in Kotlin: the reader already has each sentence's text and the preset has its `tts_locale`, so Rust needs no new calls.
    - **What is spoken:** the tapped word (a speaker button in the word drawer) and one sentence (a play button on each sentence row, next to translate). No whole-chapter playback in v1.
    - **While a sentence plays,** its row is tinted; no auto-scroll. Tapping play again, or another play button, stops it first. Leaving the reader stops speech.
    - **Settings:** a speed slider (0.5× to 1.5×); a "Speak words when tapped" switch, off by default; and a voice picker per language in the library, listing the installed voices for that language with a test button. Without a choice, the best installed voice for the preset's locale is used, offline voices first.
    - **No voice installed** for the book's language: the speaker buttons explain this and open Android's text-to-speech settings to download one.
    - **Built for a cloud voice later:** a `Speech` interface (speak word, speak sentence, stop, voices) with the phone's TTS behind it, so a provider such as Google Cloud or Azure can be added as another implementation.
    - **Anki cards** keep Anki's `{{tts}}` tag; no audio files for now (revisit with cloud voices).
    - **Done when** on a phone: the word button and sentence buttons speak in the book's language at the chosen speed and voice, the playing row is tinted, and a missing voice leads to the TTS settings. M4 is complete.
26. *(Deferred past v1, backup included, 2026-11-25; see the roadmap.)* **Library, stats and backup.** Browse Standard Ebooks and Project Gutenberg through their OPDS catalogues (parsed in Rust with `quick-xml`) and download books in the chosen language. Add a stats screen (F25), and back up and restore everything as one zip file (F27).
    - **Done when** you can find, download and start a new French book without leaving the app.
26a. **Make the repository presentable.** *Added 2026-11-25, after the repository went public.* Before v1.0, once the screens have settled:
    - **GitHub page:** a one-line description, topics (android, rust, language-learning, epub, anki, uniffi, kotlin, jetpack-compose), and the latest release linked.
    - **README:** what it is and who it is for, the feature list, screenshots (library, reader with colours, word drawer, Settings), how to install from Releases, the privacy promise (no telemetry; keys stay on the phone), how it works (Rust core, UniFFI, Compose), how to build, the CI badge, and credits (Lukaisu presets, Wiktionary and kaikki.org under CC BY-SA, DeepL, AnkiDroid).
    - **Screenshots:** taken on a phone, or with Compose screenshot tests, in light and dark mode, stored in `docs/screenshots/`.
    - **Repository files:** `CONTRIBUTING.md` (how to build, the checks to run, conventional commits), issue templates for bugs and feature ideas, and a short `SECURITY.md` on how to report a problem.
    - **Done when** someone landing on the GitHub page can tell what the app does, see it, and install it in a few minutes.
27. **Release.** Set up a release keystore and signing, reproducible-build settings, and fastlane metadata with screenshots.
    - Publish signed APKs on GitHub releases. F-Droid was dropped on 2026-11-24.
    - **Done when** the app installs from a GitHub release APK. M5 is complete, and v1.0 is out.
