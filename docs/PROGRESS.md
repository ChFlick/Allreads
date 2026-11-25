# Progress

Tick a step only when its **Done when** check in `IMPLEMENTATION_PLAN.md` passes. Add the date and any notes.

## Phone smoke test (2026-11-25)
First run of the CI APK on Christoph's phone. Working: all screens, EPUB import. Found and fixed (commit f799965): tapping a word crashed (a WordStatus in rememberSaveable); translations failed (missing INTERNET permission); image-only pages showed as empty chapters (now skipped, and old imports repair themselves); new words were too loud (now a thin, faint underline only). Added: the DeepL key is checked right after saving, with the month's usage shown. Next: re-run the smoke test with these fixes.

## Phase 0: toolchain and repo setup
- [x] 1. Install the toolchain (2026-11-24: checked in CI instead of locally. The `android-libs` job builds liballreads_ffi.so for arm64-v8a, armeabi-v7a and x86_64 at API 26 with NDK 30. Your own machine still needs the NDK for local Android builds)
- [ ] 2. Create the repo (2026-11-24: Rust workspace, docs, licence and README done. The Android project in android/ was written by hand (Compose, minSdk 26, AGP 8.13); it builds and runs its Kotlin tests on this machine. Pending: the first green CI build of the APK)
- [ ] 3. Set up CI (2026-11-24: fmt, clippy, docs, test, bindings and a Rust 1.91 job are green on main. The android job now runs assembleDebug and the Kotlin tests. Pending: its first green run)
- [x] 4. Port the language presets (2026-11-24: en, fr, de, es in data/languages; loader in crates/core/src/presets.rs checks each file and rejects duplicate codes; NOTICE added for Lukaisu attribution)

## Phase 1 (M0): Rust ↔ Kotlin spike
- [ ] 5. Hello from Rust (2026-11-25: the CI APK runs on Christoph's phone: screens work, EPUB import works, and the Rust library loads. Pending: the emulator, which the Done-when check also names)
- [x] 6. Records and errors across the boundary (2026-11-24: checked on the JVM instead of a device: a Kotlin unit test calls the real Rust library and gets a typed AllreadsException.NotFound for a missing file. Error fields are named reason, not message, which would clash with Kotlin's Exception.message)
- [ ] 7. EPUB spike (Rust side done 2026-11-24: epub::Book with meta and chapter_text, using rbook and scraper. Tested on generated fixtures, including div-only markup, NBSP and creator roles. Pending: real Gutenberg EPUB and the phone scroll check)

## Phase 2 (M1): reading MVP
- [x] 8. Database and the `App` object (2026-11-24: migrations V1 with all seven tables; repositories in crates/core/src/db with 65 core tests on an in-memory database; ffi `App` holds the open database behind a Mutex)
- [ ] 9. Book import and library (Rust side done 2026-11-24: built-in presets added on start, `App::import_book` for EPUB and TXT, table of contents (migration V2), covers saved to the data folder, `App` caches opened books. The Rust test imports three fixtures and lists them after reopening. The library screen (list, covers, progress, import via the file picker with a language choice, delete) is written and compiles. Pending: trying it on a phone)
- [x] 10. Text pipeline (2026-11-24: crates/core/src/text: blocks with heading levels and italic/bold spans, sentence splitting after Lukaisu's rules, tokens with French elision, and the shared term_key. insta snapshots for French, German and English are stable, and a golden test matches Lukaisu's parser fixtures. Terms are now stored by key with display_text (migration V3))
- [ ] 11. `open_chapter` (Rust side done 2026-11-24: `App::open_chapter` returns blocks of tokens with term keys, statuses and italic/bold; parses are cached (migration V4) and statuses joined fresh with one query. Host benchmark for 10,000 words: 10.8 ms cold, 6.1 ms cached, 0.2 ms status join. Pending: the 300 ms check on a phone, which also covers passing the records to Kotlin)
- [ ] 12. Reader screen (2026-11-25: one row per sentence, each with a translate button that shows DeepL's translation under it; status colours with an underline for new words, italics, bold, headings; serif text sized from Settings; taps map back to tokens (unit-tested); chapter buttons; position saved while scrolling and restored on open. Pending: the tap and resume check on a phone)
- [ ] 13. Word sheet and saving (Rust side done 2026-11-24: `App::save_term`, `find_term`, `forget_term`, and `mark_rest_known` for F8. A test saves a word and checks every occurrence is recoloured on the next `open_chapter`. The word drawer opens half-height with the word, DeepL's translation in context, status chips and Save; pulled up it shows the sentence, notes and a web dictionary link. "Mark the rest of this chapter as known" is in the reader menu. Pending: the recolour check on a phone)
- [ ] 14. Translator (Rust side done 2026-11-24: `Translator` trait with a DeepL client over ureq, checked against DeepL's OpenAPI spec; words are translated with their sentence as context; results cached per text, context, language pair and provider (migration V5); busy and server errors are retried. Tested against a local mock server, including errors and a second call served from the cache with no request. Pending: a real DeepL key and the under-1 s check on a phone)
- [ ] 15. Settings and polish (2026-11-25: Settings tab with the DeepL key (encrypted with an Android Keystore AES key, only its last four characters shown), native language, text size, line spacing with a preview, and System/Light/Dark theme with darker status colours in dark mode. A Words tab lists saved words with search, status filters and counts (F17). Pending: reading a full chapter on a phone)

## Phase 3 (M2): AnkiDroid integration
- [x] 17a. Anki cards in the core (2026-11-25: words keep their sentence, its translation and the book/chapter (migration V7); one note type per language with a spoken sentence card via `{{tts}}` and an optional word card; notes built with the word in bold and HTML escaped; levels 1–4 queue a card, a word with a card is re-queued on edit; deck and note type ids stored in a `setting` table; FFI calls for Kotlin. The reader passes the sentence and the translation already shown. Pending: the AnkiDroid side, steps 16–19)
- [x] 16. Connect to AnkiDroid (2026-11-25: `AnkiBridge` interface on AnkiDroid's API library (JitPack), the permission and `<queries>` entry, and a Settings section showing Install / Grant access / Connected with a deck picker and "Create Allreads deck". Checked on a phone 2026-11-25)
- [x] 17. Note type and deck (2026-11-25: `AnkiNoteTypes.ensure` creates "Allreads (Language)" from the Rust definition on first use, and again if deleted in Anki, and stores its id; Settings creates it for every library language once a deck is chosen, lists them, and has the optional word card switch; JVM tests with a fake AnkiDroid. Checked on a phone 2026-11-25)
- [x] 18. Push and update (2026-11-25: `AnkiSync.drain()` pushes queued words after each save, when the app resumes and when a deck is chosen; it updates a word's card, reuses a note with the same Word after a forget and re-save, re-adds a card deleted in Anki, and leaves failures queued; six JVM tests with a fake AnkiDroid. Checked on a phone 2026-11-25)
- [ ] 19. Outbox and retry (2026-11-25: Settings shows "N words waiting for AnkiDroid" with "Send now"; the queue drains after saves, on resume (including after the permission prompt) and when a deck is chosen; failed attempts are counted. Pending: saving words with AnkiDroid uninstalled, reinstalling it, and seeing them arrive)
- [-] 20. ~~`.apkg` and CSV export~~ (dropped 2026-11-25)

## Phases 4–6 (M3–M5)
- [ ] 21. Sentence translation and parallel mode
- [ ] 22. AI "Explain"
- [ ] 23. Offline dictionaries
- [ ] 24. Phrase selection
- [ ] 25. Read-aloud (2026-11-25: a `Speech` interface with Android TTS behind it; a play button per sentence row (its row tinted while it speaks, no auto-scroll) and in the word drawer; Settings has a speed slider, "Speak words when tapped" and a voice picker per library language with a test button; a missing voice offers the TTS settings; JVM tests for choosing a voice. Pending: checking on a phone)
- [ ] 26. Library, stats and backup
- [ ] 27. Release
