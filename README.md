# Allreads

An open-source Android app for learning languages by reading books. Import an EPUB or TXT, read it with words coloured by how well you know them, tap a word to look it up, and save it. Saved words go to AnkiDroid for review.

## Status

Early development. Steps 1 to 4 are done, and the Rust side of steps 5 to 7 (UniFFI bridge, EPUB reading). See `docs/PROGRESS.md`.

## Layout

- `crates/core`: domain logic (books, text, vocabulary, storage)
- `crates/ffi`: UniFFI wrappers for Kotlin (and later Swift)
- `android/`: Compose app; Gradle builds the Rust library and its Kotlin bindings
- `data/languages/`: language presets
- `docs/`: plan, PRD, implementation plan, progress, decisions

## Development

Minimum supported Rust version: 1.91.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
```

Test fixtures live in `crates/core/tests/fixtures`. Regenerate them with `python3 make_fixtures.py` in that folder.

Kotlin bindings for the Android app:

```bash
cargo build -p allreads-ffi
cargo run -p allreads-ffi --bin uniffi-bindgen -- generate \
  --library target/debug/liballreads_ffi.so --language kotlin --out-dir target/bindings
```

## Licence

MIT. See `LICENSE`.
