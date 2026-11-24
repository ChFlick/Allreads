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
| 2026-11-24 | Pin rusqlite to 0.39 | refinery 0.9 (the newest that works with our Rust 1.91 minimum) only supports rusqlite up to 0.39. refinery 0.10 needs Rust 1.92 |
| 2026-11-24 | Deleting a term removes its AnkiDroid link and queue entry, but the card stays in AnkiDroid | Closes the open question from the plan: deleting in Allreads never deletes the user's Anki card. Status sync back from Anki is still out of scope |
| 2026-11-24 | Status is stored as a number with a CHECK constraint (1 to 5, 98, 99); a word with no row is "new" | Matches the plan's status model and stops bad values at the database, not just in Rust |
| 2026-11-24 | Rust work runs ahead of the plan order while Android Studio is unavailable; device and Gradle checks are batched | Keeps progress going. The risk (an unproven Android build) is reduced by building the Android libraries in CI next |
| 2026-11-24 | Term keys are lower case, built by one shared `term_key` function (NFC, preset substitutions, lower case, collapsed whitespace); the first-seen form is kept for display | "Le" and "le" must be one term, and ’ and ' must match. German nouns need their capital letter on the card |
| 2026-11-24 | Drop the F-Droid requirement; release APKs on GitHub | Chosen by Christoph. Dependencies no longer need to be F-Droid-safe |
| 2026-11-24 | EPUB parsing uses `rbook` (not the `epub` crate) | Already in use, maintained, handles EPUB 2 and 3, and thread-safe by default |
| 2026-11-24 | HTTP uses the blocking `reqwest` client with rustls, and no `tokio` | The core is synchronous and Kotlin calls it on `Dispatchers.IO`, so an async runtime adds size and complexity for nothing |
| 2026-11-24 | Only terms saved with status 1–4 become Anki cards; PRD goal G2 now says so | Matches F20. Words saved as 5, ignored or known need no review |
| 2026-11-24 | The note type has both card templates (word → meaning, sentence → meaning) | Closes the PRD open question, as step 17 already specified |
| 2026-11-24 | Use `ureq` (rustls) for HTTP instead of blocking `reqwest`; supersedes the earlier blocking-`reqwest` row | `reqwest`'s blocking client still runs a hidden tokio runtime on a background thread. `ureq` is purely blocking, smaller, and has fewer dependencies, with the same calling style |
| 2026-11-24 | Built-in languages are added on start only if missing, never overwritten | Settings the user changes later (step 15) must survive app restarts |
| 2026-11-24 | Chapter titles come from each chapter's first heading; a chapter without one has no title, and the app shows "Chapter N" | Simple and works for EPUB 2 and 3. The EPUB's own navigation can replace it later if headings turn out to be poor |
| 2026-11-24 | Import is split into `prepare_import` (reads the file, no database) and `store_import` (writes) | Parsing a big book must not block other reads while the database lock is held |
| 2026-11-24 | Deleting a book removes its file only if the file is inside the app's data folder | The app must never delete a file it did not copy in |
| 2026-11-24 | The tokenizer is our own clear Rust scan, not a line-by-line port of Lukaisu's marker-string parser; Lukaisu's fixtures are the test | Easier to read and extend. It matches Lukaisu's sentences and words, with three documented differences: an abbreviation's dot is not part of the word, numbers are not words, and non-Latin scripts such as Hebrew are words |
| 2026-11-24 | Elision is a preset setting (`elision_prefixes`, French: `l\|d\|j\|m\|n\|s\|t\|c\|qu\|jusqu\|lorsqu\|puisqu\|quoiqu`); words joined by an apostrophe stay whole unless the part before it is listed | "l'homme" becomes "l'" + "homme", while "aujourd'hui" and English "don't" stay whole. Italian can reuse it later |
| 2026-11-24 | A closing mark after a space (`? »`) stays with its sentence, but only for unambiguous closers (» › ” ’ ) ] 」 』) | French puts a space before ». The opening « and the straight " could start the next sentence instead |
| 2026-11-24 | The chapter cache stores JSON; a binary format (e.g. `postcard`) is an option if the phone check needs it | Simple and readable while debugging. On the host, reading the cache is 6.1 ms against 10.8 ms for a fresh parse, so a binary format could save a few more milliseconds |
| 2026-11-24 | Statuses are joined from all saved terms of the language in one query, not an `IN (...)` list of the chapter's words | One simple query, with no SQLite parameter limit to worry about; 0.2 ms for a 10,000-word chapter against 3,000 terms |
| 2026-11-24 | A token takes the style (italic, bold) of the span where it starts | A word split across styles is rare, and splitting the token would break its term key |
