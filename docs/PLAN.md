# Reading-first language app: plan

Nov 23, 2026 · Christoph Butz

## Recommendation

Build our own Android app: a Rust core for books, text and data, a thin Kotlin + Jetpack Compose UI, and AnkiDroid for reviews. Lukaisu is a reference and a source of ideas, not the base.

- **A different shape.** Lukaisu is a WebView port of a PHP server app (about 35 pages, a schema that mirrors MySQL). We want a native, book-first reader.
- **Missing core features.** It has no EPUB, sentence translation, AI or Anki integration.
- **Learning goal.** A Rust core is real systems work (parsing, tokenizing, SQLite, HTTP), and UniFFI keeps the Kotlin side small.
- **iOS stays possible.** UniFFI also generates Swift bindings, so a later iOS app reuses the whole core.
- **No review engine.** Saved words go to AnkiDroid through its API, and Anki handles scheduling and sync.

## What Lukaisu has today

[Lukaisu](https://github.com/HugoFara/lukaisu) is an Android app that works fully offline. You paste in a text, and the app colour-codes each word, saves translations and runs FSRS reviews. It has about 48k lines of TypeScript and Svelte, 76 test files, and an optional companion server for CJK word splitting, TTS and text discovery.

| Area | State | Worth taking? |
| --- | --- | --- |
| Parsers (`offline/local/parser/`) | Regex, character and `Intl.Segmenter` tokenizers, plus sentence-end rules | Yes, as a spec and test cases for the Rust port |
| Language presets and seed content | Per-language split rules, RTL flags, dictionary URLs | Yes, as data (convert to JSON or TOML) |
| Word status model | 1–5 = learning, 98 = ignored, 99 = well known | Yes, users of LWT, Lute and LingQ will recognise it |
| FSRS integration | Built-in review | No, AnkiDroid does reviews |
| Data layer (Dexie + `routeLocal`) | Imitates `/api/v1` server routes on the device | No |
| UI (about 35 pages, Bulma, Alpine leftovers) | Works, but legacy in shape | Ideas for screens only |
| Release tooling | Own F-Droid repo, reproducible-build notes | No: F-Droid was dropped (2026-11-24) |

Its roadmap is mostly about F-Droid release and the local-first switch. Sync, offline CJK, TTS and content discovery are deferred or out of scope, and there is no mention of EPUB or AI.

## Target feature set

The aim is [Smart Book](https://smart-book.net/)'s reading experience with LingQ-style vocabulary tracking, open source, and with no paid tier needed. AI is optional and uses your own API key.

| Feature | Smart Book | Lukaisu | Ours (v1) |
| --- | --- | --- | --- |
| EPUB, TXT and subtitle (SRT) import | Yes | Text only | EPUB and TXT (SRT later) |
| Free book library | 75k+ built in | No | Browse Project Gutenberg and Standard Ebooks |
| Tap a word for its translation | Yes | Yes | Yes |
| Word status colours and known-word count | No | Yes | Yes |
| Sentence and parallel translation | Yes | No | Yes, toggled per book |
| AI explanations (grammar, idioms, context) | Paid | No | Bring your own key |
| Review | Basic, in-app | FSRS, in-app | AnkiDroid (cards pushed through its API) |
| Anki | Export to file | No | Live push to AnkiDroid, plus `.apkg` export |
| Read-aloud and pronunciation | Yes | Server only | Android system TTS, offline |
| Offline translation | Yes | No | Later (Bergamot) |
| Platforms | iOS, Android | Android | Android (iOS later from the same Rust core) |

## Tech stack

All the logic lives in a Rust library, built for Android with `cargo-ndk`. UniFFI generates Kotlin bindings for it, so the Compose UI calls Rust like any Kotlin library, including `suspend` functions for async work. Kotlin only does what needs Android itself: the UI, TTS, the AnkiDroid bridge, file picking and key storage.

| Layer | Choice | Side | Why |
| --- | --- | --- | --- |
| UI | Jetpack Compose + Material 3, Navigation Compose | Kotlin | Native rendering and accessibility, with tappable words drawn directly |
| Bindings | UniFFI (Mozilla) | Both | Generates Kotlin now and Swift later from one interface |
| Build | `cargo-ndk` called from a Gradle task | Both | Builds `.so` libraries for arm64, armv7 and x86_64 |
| EPUB parsing | `rbook` (or `epub`) + `scraper` for XHTML | Rust | Chapters become clean paragraphs, no WebView needed |
| Tokenizing | `unicode-segmentation`, plus the ported Lukaisu rules; `lindera` (Japanese) and `jieba-rs` (Chinese) later | Rust | Offline and fast, CJK without a server |
| Storage | `rusqlite` (bundled SQLite) + `refinery` migrations | Rust | One database file, easy to back up |
| HTTP | `reqwest` (rustls) + `tokio` | Rust | Translator and AI clients |
| Translation | `Translator` trait: DeepL, LibreTranslate, LLM | Rust | Swappable; Bergamot offline later |
| AI | `AiProvider` trait: OpenAI-compatible + Anthropic, bring your own key | Rust | No backend to run; answers cached per sentence |
| Dictionaries | Wiktionary extracts (kaikki.org, CC BY-SA), downloaded per language | Rust | Offline, with lemmas and inflections |
| Anki | AnkiDroid `AddContentApi` | Kotlin | Officially supported; AnkiDroid does scheduling and AnkiWeb sync |
| `.apkg` export | `genanki-rs` | Rust | Fallback for people without AnkiDroid |
| Text-to-speech | Android `TextToSpeech` | Kotlin | System voices, offline |
| API keys | Android Keystore (encrypted storage) | Kotlin | Keys never stored in plain text |
| Tests | `cargo test` + `insta` snapshots; Compose UI tests | Both | Most logic is tested in Rust, without an emulator |

## Architecture and data model

The Rust core owns all state and rules. The Kotlin app is a shell that renders what the core returns and forwards taps, so almost everything can be unit-tested in Rust.

```
allreads/
  Cargo.toml              # Rust workspace
  crates/
    core/                 # domain: books, text, terms, db, assist
    ffi/                  # UniFFI exports, thin wrappers over core
  android/
    app/                  # Compose UI, TTS, AnkiDroid bridge
    build.gradle.kts      # calls cargo-ndk, generates bindings
  data/languages/         # presets ported from Lukaisu
```

1. **Compose UI**: Library, Reader, Word sheet, Vocabulary, Settings.
2. **Kotlin platform services**: `AnkiBridge` (AnkiDroid API), `Speech` (TTS), `KeyStore`, file picker.
3. **FFI layer** (`crates/ffi`): an `App` object with methods such as `import_book`, `open_chapter`, `lookup`, `save_term` and `explain`, and plain records that cross the boundary.
4. **Core modules** (`crates/core`): `epub`, `text` (tokenizing and sentences), `db` (schema and repositories), `terms` (statuses and lemmas), `assist` (translate, explain, cache), `export` (`.apkg`).

Opening a chapter, the Kotlin side calls `open_chapter(book, index)`. Rust parses it (or reads a cached parse), tokenizes it, joins the tokens with the Term table, and returns paragraphs of tokens, each with its text, term key and status. Compose renders each paragraph as styled text; a tap maps back to its token.

| Entity | Key fields | Note |
| --- | --- | --- |
| Language | code, name, split rules, RTL, TTS locale | Seeded from Lukaisu's presets |
| Book | title, author, language, file path, cover, position (chapter + paragraph) | EPUB, or a TXT wrapped as one chapter |
| Term | language, text, lemma, status (1–5 / 98 / 99), translation, notes, parent term | A multi-word phrase is just a term with spaces |
| AnkiLink | term, AnkiDroid note id, deck, last pushed | Lets edits update the existing card |
| SentenceCache | hash of the sentence text, translation, AI explanation, provider | Avoids paying twice for the same sentence |
| ReadingStat | day, book, words read, new terms | Drives progress and known-word counts |

Word occurrences aren't stored; each chapter is tokenized when it's opened.

## Milestones

M1 is the real MVP: reading a French EPUB and saving words. M2 makes it complete for daily use, because the words then show up in Anki.

1. **M0: Toolchain spike.** A Rust function called from Compose through UniFFI on a real phone, then a Rust EPUB parse shown as paragraphs. This proves the whole pipeline.
2. **M1: Reading MVP.** Import EPUB and TXT, tokenize, colour words by status, tap a word for a machine translation, save it with a status, and resume where you stopped.
3. **M2: Anki.** Push saved terms to AnkiDroid with the sentence as context, update them on edit, and add `.apkg` export as a fallback.
4. **M3: Translation and AI.** Sentence translation, parallel mode, "explain this" with your own key, offline dictionaries, all cached.
5. **M4: Audio.** Read aloud with sentence highlighting, and word pronunciation.
6. **M5: Library and ship.** A Gutenberg and Standard Ebooks browser, then a signed APK on GitHub releases.
7. **Later.** iOS via Swift bindings, Japanese and Chinese tokenizers, offline translation (Bergamot), backup and sync.

## Open decisions and risks

| Item | Options | Leaning |
| --- | --- | --- |
| Licence | MIT, Apache-2.0 or GPL-3.0 | Decided: MIT |
| App name | Open | Decided: Allreads |
| Reading layout | Vertical scroll or horizontal pages | Scroll first (simpler in Compose), pages later |
| Lemmas | "mange" and "mangeons" as separate terms, or linked | Link them, using Wiktionary forms plus AI suggestions |
| Rust learning curve | Slow start on ownership and async | Keep the FFI surface small; core is synchronous where possible |
| AnkiDroid dependency | The API needs AnkiDroid installed and a permission granted | Check on launch, fall back to `.apkg` export |
| Dictionary size | Full Wiktionary extracts are hundreds of MB | Pre-filter per language pair, download on demand |
| AI cost and privacy | Text goes to the provider you pick | Off by default, bring your own key, show a cost estimate |
| Translation APIs | Scraping Google Translate breaks its terms | DeepL free tier, LibreTranslate, or an LLM |
| F-Droid rules | No proprietary libraries (no ML Kit or Play Services) | Dropped 2026-11-24: F-Droid compatibility is not required |

## Sources

- [Lukaisu repository](https://github.com/HugoFara/lukaisu): README, ROADMAP.md, CLAUDE.md and git history, read 23 Nov 2026
- [Smart Book](https://smart-book.net/): feature list
