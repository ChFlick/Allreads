# Decisions

| Date | Decision | Why |
| --- | --- | --- |
| 2026-11-23 | Build our own app instead of forking Lukaisu | Lukaisu is a WebView port of a server app; we want a native, EPUB-first reader |
| 2026-11-23 | Rust core + Kotlin/Compose UI via UniFFI | Learning Rust; a small Kotlin side; iOS possible later with Swift bindings |
| 2026-11-23 | No in-app review scheduler; push words to AnkiDroid through its API | Anki already does scheduling and sync well; officially supported API, no licence issues |
| 2026-11-23 | `.apkg` export as a fallback | For people without AnkiDroid |
| 2026-11-24 | App name: Allreads (plural), replacing the earlier "Allread"; repo: `ChFlick/Allreads` | Chosen by Christoph |
| 2026-11-24 | Licence: MIT | Chosen by Christoph; resolves the open licence question in the PRD and PLAN |
