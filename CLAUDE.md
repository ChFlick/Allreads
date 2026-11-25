# CLAUDE.md

Guidance for Claude Code working in this repository.

## What this is

An open-source Android app for learning languages by reading books (a Smart Book / LingQ-style reader). You import an EPUB, read it with words coloured by how well you know them, tap a word to look it up, and save it. Saved words are pushed to **AnkiDroid**, which handles review and sync. There is no review scheduler in this app.

Architecture: a **Rust core** (books, text, vocabulary, database, translation/AI clients) exposed to a thin **Kotlin + Jetpack Compose** UI through **UniFFI**, built with `cargo-ndk`. Android only for now; the Rust core is meant to be reusable for iOS later (UniFFI Swift bindings).

## Source of truth

Read these before starting any task:

- `docs/PLAN.md`: stack, architecture, data model, milestones, risks
- `docs/PRD.md`: goals, user stories, requirements (F1–F27), non-functional targets
- `docs/IMPLEMENTATION_PLAN.md`: the ordered steps (each with a **Done when** check) and the roadmap after v1
- `docs/PROGRESS.md`: which steps are done; update it as you go

If the code and the docs disagree, flag it and ask before changing direction. Record decisions that change the plan in `docs/DECISIONS.md` (date, decision, why).

## How we work

- **One step at a time,** in the order of `docs/IMPLEMENTATION_PLAN.md`. Start each step by restating its goal and Done-when check, and propose a short plan before writing code.
- **Core first:** logic and tests in `crates/core`, then thin wrappers in `crates/ffi`, then the Compose screen.
- **Finish a step only when its Done-when check passes.** Run `cargo fmt`, `cargo clippy -- -D warnings` and `cargo test` before saying a step is done. Then tick it in `docs/PROGRESS.md` and suggest a commit message.
- **Small commits,** one per step or sub-step, using conventional commits (`feat(core): ...`, `fix(android): ...`).
- **Commit and push without asking** once a piece of work is finished and its checks pass: commit to `main` with a conventional commit message, then push. Don't commit half-done work or failing checks.
- If something needs to be checked on a real phone (taps, performance, AnkiDroid), say so clearly instead of claiming it works.

## Christoph is learning Rust

Learning Rust is a goal of this project, so:

- Prefer clear, idiomatic code over clever code. Avoid unnecessary generics, macros and lifetimes.
- When you introduce a Rust concept for the first time (ownership, borrowing, `Result` and `?`, traits, `Arc`/`Mutex`, iterators), explain it briefly in the chat, not in code comments.
- Point out parts of a step that would be good for Christoph to write himself, and offer to review them.
- He knows TypeScript, React and Node well, so comparisons to those help.

## Constraints

- **Privacy:** no analytics or telemetry. API keys live in the Android Keystore on the Kotlin side and are passed into Rust per call; never store them in the SQLite database or in logs.
- **Small FFI surface:** a few coarse calls that return plain records (for example `open_chapter` returns a whole chapter of tokens). No per-word calls across the boundary.
- **Core code is synchronous;** Kotlin calls it on `Dispatchers.IO`. Use UniFFI async only if a step clearly needs it.
- **Licences:** Lukaisu (https://github.com/HugoFara/lukaisu) is MIT/Unlicense and may be used as a reference for parser rules, language presets and test cases. Keep attribution when porting. Don't use AGPL code (for example Anki's own `rslib`).
- minSdk 26; ABIs arm64-v8a, armeabi-v7a, x86_64.
- **The repository is public:** never commit secrets, keys, personal data or copyrighted books (test fixtures included).
- **`main` ships:** every green push to `main` publishes a pre-release APK on GitHub Releases, so keep it working. Offline dictionaries are published separately by the hand-run Dictionaries workflow.

## Layout

```
Cargo.toml              # Rust workspace
crates/
  core/                 # domain: epub, text, db, dictionary, translate, anki
  ffi/                  # UniFFI exports
  dictgen/              # builds offline dictionaries from kaikki.org extracts
android/
  app/                  # Compose UI, TTS, AnkiDroid bridge
data/languages/         # language presets (TOML)
docs/                   # plan, PRD, implementation plan, progress, decisions
```

## Commands

Minimum supported Rust is 1.91.

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --workspace
cargo +1.91 test --workspace    # same tests on the minimum Rust version

# Kotlin bindings from the host build of the library
cargo build -p allreads-ffi
cargo run -p allreads-ffi --bin uniffi-bindgen -- generate \
  --library target/debug/liballreads_ffi.so --language kotlin --out-dir target/bindings

# Offline dictionary from a kaikki.org extract (the Dictionaries workflow does this in CI)
cargo run --release -p allreads-dictgen -- --input kaikki.org-dictionary-French.jsonl.gz \
  --language fr --output fr-en.sqlite

# Test fixtures (run in crates/core/tests/fixtures)
python3 make_fixtures.py
```

Android (needs the Android SDK; `assembleDebug` also needs the NDK, with `ANDROID_NDK_HOME` set):

```bash
cd android && ./gradlew testDebugUnitTest   # Kotlin tests on the JVM, calling the host Rust build
cd android && ./gradlew assembleDebug       # APK: runs cargo-ndk and uniffi-bindgen first
cd android && ./gradlew recordPaparazziDebug --tests '*Screenshots*'   # README screenshots; copy to docs/screenshots
```
