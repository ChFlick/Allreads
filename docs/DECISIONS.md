# Decisions

| Date | Decision | Why |
| --- | --- | --- |
| 2026-11-23 | Build our own app instead of forking Lukaisu | Lukaisu is a WebView port of a server app; we want a native, EPUB-first reader |
| 2026-11-23 | Rust core + Kotlin/Compose UI via UniFFI | Learning Rust; a small Kotlin side; iOS possible later with Swift bindings |
| 2026-11-23 | No in-app review scheduler; push words to AnkiDroid through its API | Anki already does scheduling and sync well; officially supported API, no licence issues |
| 2026-11-23 | `.apkg` export as a fallback | For people without AnkiDroid |
| 2026-11-24 | App name: Allreads (plural), replacing the earlier "Allread"; repo: `ChFlick/Allreads` | Chosen by Christoph |
| 2026-11-24 | Licence: MIT | Chosen by Christoph; resolves the open licence question in the PRD and PLAN |
| 2026-11-24 | Minimum supported Rust is 1.91, checked in CI | Dependencies (uniffi, rbook, scraper, clap in the bindgen) need 1.91; the earlier 1.75 claim was wrong |
| 2026-11-24 | EPUB reading goes through `epub::Book`, which opens the file once | Re-parsing the zip on every call is too slow for chapter navigation; the step 8 `App` will hold the open book |
| 2026-11-24 | Preset files are validated and duplicate codes are rejected at load | A typo in a preset should fail loudly in tests, not silently in the app |
| 2026-11-24 | Upgrade rbook to 0.7.13, which moves zip from 7.x to 9.x | Dependabot flagged zip 7.4.0; rbook 0.6 pins zip 7.x, so the upgrade is the only route to the patched release. The only code change was dropping an unused prelude import |
