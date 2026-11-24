# Progress

Tick a step only when its **Done when** check in `IMPLEMENTATION_PLAN.md` passes. Add the date and any notes.

## Phase 0: toolchain and repo setup
- [x] 1. Install the toolchain (2026-11-24: checked in CI instead of locally. The `android-libs` job builds liballreads_ffi.so for arm64-v8a, armeabi-v7a and x86_64 at API 26 with NDK 30. Your own machine still needs the NDK for local Android builds)
- [ ] 2. Create the repo (2026-11-24: Rust workspace, docs, licence and README done. Pending: the Android project)
- [ ] 3. Set up CI (2026-11-24: fmt, clippy, docs, test, bindings and a Rust 1.91 job are green on main. Pending: the Gradle assembleDebug job)
- [x] 4. Port the language presets (2026-11-24: en, fr, de, es in data/languages; loader in crates/core/src/presets.rs checks each file and rejects duplicate codes; NOTICE added for Lukaisu attribution)

## Phase 1 (M0): Rust ↔ Kotlin spike
- [ ] 5. Hello from Rust (Rust side done 2026-11-24: uniffi scaffolding, core_version, uniffi-bindgen generates Kotlin on the host. Pending: Gradle task, phone and emulator check)
- [ ] 6. Records and errors across the boundary (Rust side done 2026-11-24: BookMeta record, AllreadsError with typed variants. Pending: check the typed Kotlin exception on a device)
- [ ] 7. EPUB spike (Rust side done 2026-11-24: epub::Book with meta and chapter_text, using rbook and scraper. Tested on generated fixtures, including div-only markup, NBSP and creator roles. Pending: real Gutenberg EPUB and the phone scroll check)

## Phase 2 (M1): reading MVP
- [x] 8. Database and the `App` object (2026-11-24: migrations V1 with all seven tables; repositories in crates/core/src/db with 65 core tests on an in-memory database; ffi `App` holds the open database behind a Mutex)
- [ ] 9. Book import and library (Rust side done 2026-11-24: built-in presets added on start, `App::import_book` for EPUB and TXT, table of contents (migration V2), covers saved to the data folder, `App` caches opened books. The Rust test imports three fixtures and lists them after reopening. Pending: the file picker and library screen)
- [x] 10. Text pipeline (2026-11-24: crates/core/src/text: blocks with heading levels and italic/bold spans, sentence splitting after Lukaisu's rules, tokens with French elision, and the shared term_key. insta snapshots for French, German and English are stable, and a golden test matches Lukaisu's parser fixtures. Terms are now stored by key with display_text (migration V3))
- [ ] 11. `open_chapter` (Rust side done 2026-11-24: `App::open_chapter` returns blocks of tokens with term keys, statuses and italic/bold; parses are cached (migration V4) and statuses joined fresh with one query. Host benchmark for 10,000 words: 10.8 ms cold, 6.1 ms cached, 0.2 ms status join. Pending: the 300 ms check on a phone, which also covers passing the records to Kotlin)
- [ ] 12. Reader screen
- [ ] 13. Word sheet and saving (Rust side done 2026-11-24: `App::save_term`, `find_term`, `forget_term`, and `mark_rest_known` for F8. A test saves a word and checks every occurrence is recoloured on the next `open_chapter`. Pending: the word sheet on Android)
- [ ] 14. Translator (Rust side done 2026-11-24: `Translator` trait with a DeepL client over ureq, checked against DeepL's OpenAPI spec; words are translated with their sentence as context; results cached per text, context, language pair and provider (migration V5); busy and server errors are retried. Tested against a local mock server, including errors and a second call served from the cache with no request. Pending: a real DeepL key and the under-1 s check on a phone)
- [ ] 15. Settings and polish

## Phase 3 (M2): AnkiDroid integration
- [ ] 16. Connect to AnkiDroid
- [ ] 17. Note type and deck
- [ ] 18. Push and update
- [ ] 19. Outbox and retry
- [ ] 20. `.apkg` and CSV export

## Phases 4–6 (M3–M5)
- [ ] 21. Sentence translation and parallel mode
- [ ] 22. AI "Explain"
- [ ] 23. Offline dictionaries
- [ ] 24. Phrase selection
- [ ] 25. Audio
- [ ] 26. Library, stats and backup
- [ ] 27. Release
