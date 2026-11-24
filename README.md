# Allreads

An open-source Android app for learning languages by reading books. Import an EPUB or TXT, read it with words coloured by how well you know them, tap a word to look it up, and save it. Saved words go to AnkiDroid for review.

## Status

Early development. Step 2 of 27 (workspace scaffolding). See `docs/PROGRESS.md`.

## Layout

- `crates/core`: domain logic (books, text, vocabulary, storage)
- `crates/ffi`: UniFFI wrappers for Kotlin (and later Swift)
- `android/`: Compose app (to be created in Android Studio)
- `data/languages/`: language presets
- `docs/`: plan, PRD, implementation plan, progress, decisions

## Development

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

## Licence

MIT. See `LICENSE`.
