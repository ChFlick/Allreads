# CLAUDE.md

Guidance for Claude Code working in this repository.

## What this is

An open-source Android app for learning languages by reading books (a Smart Book / LingQ-style reader). You import an EPUB, read it with words coloured by how well you know them, tap a word to look it up, and save it. Saved words are pushed to **AnkiDroid**, which handles review and sync. There is no review scheduler in this app.

Architecture: a **Rust core** (books, text, vocabulary, database, translation/AI clients) exposed to a thin **Kotlin + Jetpack Compose** UI through **UniFFI**, built with `cargo-ndk`. Android only for now; the Rust core is meant to be reusable for iOS later (UniFFI Swift bindings).

## Source of truth

Read these before starting any task:

- `docs/PLAN.md`: stack, architecture, data model, milestones, risks
- `docs/PRD.md`: goals, user stories, requirements (F1–F27), non-functional targets
- `docs/IMPLEMENTATION_PLAN.md`: 27 ordered steps, each with a **Done when** check
- `docs/PROGRESS.md`: which steps are done; update it as you go

If the code and the docs disagree, flag it and ask before changing direction. Record decisions that change the plan in `docs/DECISIONS.md` (date, decision, why).

## How we work

- **One step at a time,** in the order of `docs/IMPLEMENTATION_PLAN.md`. Start each step by restating its goal and Done-when check, and propose a short plan before writing code.
- **Core first:** logic and tests in `crates/core`, then thin wrappers in `crates/ffi`, then the Compose screen.
- **Finish a step only when its Done-when check passes.** Run `cargo fmt`, `cargo clippy -- -D warnings` and `cargo test` before saying a step is done. Then tick it in `docs/PROGRESS.md` and suggest a commit message.
- **Small commits,** one per step or sub-step, using conventional commits (`feat(core): ...`, `fix(android): ...`).
- If something needs to be checked on a real phone (taps, performance, AnkiDroid), say so clearly instead of claiming it works.

## Christoph is learning Rust

Learning Rust is a goal of this project, so:

- Prefer clear, idiomatic code over clever code. Avoid unnecessary generics, macros and lifetimes.
- When you introduce a Rust concept for the first time (ownership, borrowing, `Result` and `?`, traits, `Arc`/`Mutex`, iterators), explain it briefly in the chat, not in code comments.
- Point out parts of a step that would be good for Christoph to write himself, and offer to review them.
- He knows TypeScript, React and Node well, so comparisons to those help.

## Constraints

- **F-Droid-safe:** every dependency must be open source. No Google Play Services, ML Kit, Firebase or proprietary SDKs.
- **Privacy:** no analytics or telemetry. API keys live in the Android Keystore on the Kotlin side and are passed into Rust per call; never store them in the SQLite database or in logs.
- **Small FFI surface:** a few coarse calls that return plain records (for example `open_chapter` returns a whole chapter of tokens). No per-word calls across the boundary.
- **Core code is synchronous;** Kotlin calls it on `Dispatchers.IO`. Use UniFFI async only if a step clearly needs it.
- **Licences:** Lukaisu (https://github.com/HugoFara/lukaisu) is MIT/Unlicense and may be used as a reference for parser rules, language presets and test cases. Keep attribution when porting. Don't use AGPL code (for example Anki's own `rslib`).
- minSdk 26; ABIs arm64-v8a, armeabi-v7a, x86_64.

## Layout (target)

```
Cargo.toml              # Rust workspace
crates/
  core/                 # domain: epub, text, db, terms, assist, export
  ffi/                  # UniFFI exports
android/
  app/                  # Compose UI, TTS, AnkiDroid bridge
data/languages/         # language presets (TOML)
docs/                   # plan, PRD, implementation plan, progress, decisions
```

## Commands

Fill these in as they're created (step 2 onwards):

```bash
cargo test                      # core tests
cargo clippy -- -D warnings
cargo ndk -t arm64-v8a build    # Android build of the Rust library
cd android && ./gradlew assembleDebug
```
