# Progress

Tick a step only when its **Done when** check in `IMPLEMENTATION_PLAN.md` passes. Add the date and any notes.

## Phase 0: toolchain and repo setup
- [ ] 1. Install the toolchain
- [ ] 2. Create the repo
- [ ] 3. Set up CI
- [x] 4. Port the language presets (2026-11-24: en, fr, de, es in data/languages; loader and test in crates/core/src/presets.rs; NOTICE added for Lukaisu attribution)

## Phase 1 (M0): Rust ↔ Kotlin spike
- [ ] 5. Hello from Rust (Rust side done 2026-11-24: uniffi scaffolding, core_version, uniffi-bindgen generates Kotlin on the host. Pending: Gradle task, phone and emulator check)
- [ ] 6. Records and errors across the boundary (Rust side done 2026-11-24: BookMeta record, AllreadsError with typed variants. Pending: check the typed Kotlin exception on a device)
- [ ] 7. EPUB spike (Rust side done 2026-11-24: read_meta and chapter_text with rbook and scraper, tested on a generated French fixture. Pending: real Gutenberg EPUB and the phone scroll check)

## Phase 2 (M1): reading MVP
- [ ] 8. Database and the `App` object
- [ ] 9. Book import and library
- [ ] 10. Text pipeline
- [ ] 11. `open_chapter`
- [ ] 12. Reader screen
- [ ] 13. Word sheet and saving
- [ ] 14. Translator
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
