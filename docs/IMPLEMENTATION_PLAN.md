# Implementation plan

Nov 23, 2026 · Christoph Butz

## How to use this plan

There are 27 steps in 6 phases, and each step is sized for an evening or two. Each one ends with a **Done when** check, and most say which Rust concept it teaches.

- Do the steps in order; each one builds on the last. Phase 2 ends with the MVP and phase 3 with daily-use Anki.
- Write logic and tests in `crates/core` first, expose it through `crates/ffi`, then build the screen.
- Keep the FFI surface small: a few coarse calls returning plain records, never chatty per-word calls.
- Core code is synchronous; Kotlin calls it on `Dispatchers.IO`. Move to UniFFI async only if a step needs it.
- References: [The Rust Book](https://doc.rust-lang.org/book/), [Rustlings](https://github.com/rust-lang/rustlings), the [UniFFI user guide](https://mozilla.github.io/uniffi-rs/) and the [AnkiDroid API wiki](https://github.com/ankidroid/Anki-Android/wiki/AnkiDroid-API).

## Phase 0: toolchain and repo setup

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
10. **Text pipeline.** Build `core::text`: XHTML becomes blocks (heading or paragraph, with italic and bold spans). Then split sentences and tokenize words with `unicode-segmentation` plus the preset rules (French elision "l'homme" → "l'" + "homme", hyphens, numbers).
    - Use Lukaisu's parser tests as cases, written as `insta` snapshots.
    - *Learn:* iterators, traits, `&str` versus `String`.
    - **Done when** the snapshots for French, German and English sample texts are stable.
11. **`open_chapter`.** Return `Chapter { blocks: [Block { kind, tokens: [Token { text, is_word, term_key, status, sentence }] }] }`. Join tokens with terms through one query into a `HashMap`, and cache the parsed chapter as a blob.
    - Add a `criterion` benchmark.
    - **Done when** a 10,000-word chapter opens in under 300 ms on your phone.
12. **Reader screen.** Render each block in a `LazyColumn` as an `AnnotatedString`, with a background style per status. On tap, use `pointerInput` + `TextLayoutResult.getOffsetForPosition` to find the character offset, then the token. Save the scroll position as chapter + block.
    - **Done when** taps hit the right word and reopening the book resumes in place.
13. **Word sheet and saving.** A `ModalBottomSheet` shows the word, its sentence, status chips (1–5, known, ignore) and a translation field. `app.save_term(...)` saves it, then the chapter's statuses are refreshed without a full reload.
    - **Done when** saving a word recolours every occurrence in the chapter.
14. **Translator.** Add a `Translator` trait with DeepL and LibreTranslate implementations (`reqwest` blocking, rustls), and cache results in SentenceCache. The key lives in the Android Keystore and is passed in per call.
    - *Learn:* trait objects, `Box<dyn Trait>`, HTTP and `serde_json`.
    - **Done when** tapping a new word shows a translation in under 1 s, and instantly the second time.
15. **Settings and polish.** Add language setup from presets, translator settings, font size, line spacing, dark theme, and "mark rest of chapter as known".
    - **Done when** you read a full chapter of a French novel on your phone and save words without friction. M1 is complete.

## Phase 3 (M2): AnkiDroid integration

16. **Connect to AnkiDroid.** Add AnkiDroid's API library (`com.ichi2.anki.api`), the `com.ichi2.anki.permission.READ_WRITE_DATABASE` permission, and a `<queries>` entry for `com.ichi2.anki` (needed on Android 11+). Detect whether AnkiDroid is installed and request the runtime permission.
    - **Done when** the settings screen shows "Connected to AnkiDroid" or a clear install prompt.
17. **Note type and deck.** On first use, create the app's note type with `AddContentApi.addNewCustomModel`:
    - fields: Word, Lemma, Translation, Sentence (with the word in bold), Sentence translation, Source;
    - two card templates: word → meaning, and sentence → meaning.
    - Pick or create a deck, and store both ids in settings.
    - **Done when** the note type and deck appear in AnkiDroid.
18. **Push and update.** Rust builds an `AnkiNote` record (HTML fields, sentence with `<b>word</b>`, book and chapter as the source). Kotlin's `AnkiBridge` calls `addNote`, or `updateNoteFields` if an AnkiLink exists, and records the note id through `app.record_anki_link`.
    - Push on save for statuses 1–4; never create duplicates.
    - **Done when** editing a term's translation updates the same card.
19. **Outbox and retry.** When AnkiDroid isn't available, the push goes into `anki_outbox` in Rust. Drain the queue on app start and after permission is granted.
    - **Done when** words saved with AnkiDroid uninstalled arrive after it's reinstalled.
20. **`.apkg` and CSV export.** Build the deck with `genanki-rs` using the same note type, and share it through the Android share sheet.
    - **Done when** the file imports cleanly into desktop Anki. M2 is complete: save 20 words while reading, sync AnkiDroid, and review them on desktop.

## Phases 4–6 (M3–M5): translation and AI, audio, library and release

21. **Sentence translation and parallel mode.** Add `translate_sentence` with caching. Parallel mode renders a muted translation under each sentence and pre-translates the visible blocks in the background.
    - **Done when** parallel mode stays smooth while scrolling a chapter.
22. **AI "Explain".** Add an `AiProvider` trait with OpenAI-compatible (which covers many hosts, including local models) and Anthropic implementations. The prompt includes the target and native language, the sentence and the word, and asks for meaning in context, grammar and why this form. Answers are cached per sentence + word, and the sheet shows a cost estimate before the call.
    - **Done when** "Explain" answers in the sheet and repeats are free.
23. **Offline dictionaries.** A separate binary, `crates/dictgen`, filters kaikki.org Wiktionary JSONL into a compact SQLite file per language pair, published as a GitHub release asset. The app downloads it on demand and uses it for lookups and lemma linking (F12, F16).
    - *Learn:* streaming large files, CLI tools with `clap`.
    - **Done when** French → English lookups work in flight mode.
24. **Phrase selection.** Long-press and drag across tokens to select a phrase, saved as a multi-word term. Phrases are matched in `open_chapter` (longest match first).
    - **Done when** a saved idiom is highlighted wherever it appears.
25. **Audio.** Use Android `TextToSpeech` with an `UtteranceProgressListener`. Rust supplies sentence ranges, and the reader highlights the sentence being spoken. Add a speak button in the word sheet.
    - **Done when** "read chapter aloud" follows along correctly. M4 is complete.
26. **Library, stats and backup.** Browse Standard Ebooks and Project Gutenberg through their OPDS catalogues (parsed in Rust with `quick-xml`) and download books in the chosen language. Add a stats screen (F25), and back up and restore everything as one zip file (F27).
    - **Done when** you can find, download and start a new French book without leaving the app.
27. **Release.** Set up a release keystore and signing, reproducible-build settings, and fastlane metadata with screenshots.
    - Publish to your own F-Droid repo first (like Lukaisu), then submit to the main F-Droid catalogue, and publish APKs on GitHub releases.
    - **Done when** the app installs from F-Droid. M5 is complete, and v1.0 is out.
