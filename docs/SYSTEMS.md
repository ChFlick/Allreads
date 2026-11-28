# How Allreads' systems work

What each part builds, where its data comes from, what is in its files, and how the app uses them. The plan and the reasons behind choices are in `PLAN.md`, `IMPLEMENTATION_PLAN.md` and `DECISIONS.md`; this file describes how things work today.

## The big picture

```
GitHub Actions                         The phone
──────────────                         ─────────
CI ─────────────► APK (Releases) ───► Allreads app (Kotlin + Compose)
                                          │  UniFFI: a few coarse calls
Dictionaries ───► dictionaries release ─► Rust core (crates/core)
  (hand-run)       • fr-en.sqlite.gz         ├─ allreads.db (SQLite)
                   • dictionaries.json       ├─ books/
                                             ├─ dictionaries/
(book lists:       • fr.sqlite.gz            └─ catalogs/
 not built yet)    (listed in the same json)
```

- **`crates/core`**: all logic and storage: books, text, words, counting, dictionaries, book lists, the vocabulary check, DeepL, Anki note building.
- **`crates/ffi`**: thin UniFFI wrappers. Calls are coarse (a whole chapter, a whole book list, one band of check answers), never one per word.
- **`crates/dictgen`**: the offline builders, run by GitHub Actions: `allreads-dictgen` (dictionaries, word frequencies) and `catalog` (book lists).
- **`android/app`**: the Compose UI, text-to-speech, the AnkiDroid bridge, downloads and settings.

Everything the app knows about the reader (saved words, levels, reading positions, check results) stays in `allreads.db` on the phone. Nothing is sent anywhere except DeepL translations (with the reader's own key), AnkiDroid cards (on the same phone), and the downloads below.

### Where the app keeps its data

In the app's private data folder:

| Path | What |
|---|---|
| `allreads.db` | The database: books, chapters, saved words, Anki links and queue, settings, progress, counts (migrations `crates/core/migrations`) |
| `books/` | Imported book files (EPUB, TXT) and covers |
| `dictionaries/<pair>.sqlite` | Installed offline dictionaries, e.g. `fr-en.sqlite` |
| `catalogs/<language>.sqlite` | Installed book lists for "Find a book", e.g. `fr.sqlite` |

API keys are not in there: they live in the Android Keystore and are passed into Rust per call.

## Builds and releases

| Trigger | Workflow | What it publishes |
|---|---|---|
| Push to `main` | `ci.yml` | All checks (Rust fmt, clippy, docs, tests on stable and 1.91; Kotlin tests; lint), then the APK as a pre-release `v<version>-build.<n>` on GitHub Releases |
| Push to `feature/…` | `ci.yml` | The same checks; the APK ("Allreads Preview") only as a workflow artifact (Actions → the run → Artifacts, 90 days) |
| Tag `preview-*` | `ci.yml` | That build as a pre-release "Preview: …" |
| By hand | `dictionaries.yml` | Offline dictionaries with word frequencies, and the manifest, to the `dictionaries` release |

- APKs are debug builds signed with the fixed key in `android/app`, so each one installs over the last.
- "Allreads Preview" (`io.github.chflick.allreads.preview`) installs next to the normal app with its own data, so a branch's database changes never touch the real library.

## Offline dictionaries

**Source.** Wiktionary, via kaikki.org's machine-readable extracts (CC BY-SA 4.0).

**Build** (`dictionaries.yml`, run by hand from the Actions tab with a list of language codes):
1. Download `kaikki.org-dictionary-<Language>.jsonl.gz`.
2. `allreads-dictgen --input … --language fr --output fr-en.sqlite` keeps the language's entries, turns each headword into the app's term key (the same key the reader gives a tapped word), and stores meanings, synonyms and "form of" links.
3. Add word frequencies to the same file (see below).
4. Gzip, compute the SHA-256, and write the manifest.
5. Upload each `<pair>.sqlite.gz`, then `dictionaries.json`, to the `dictionaries` release. The book lists the published manifest already lists are kept.

**File** (SQLite, schema in `crates/core/src/dictionary/schema.sql`):

| Table | What |
|---|---|
| `meta` | `format`, `pair` (`fr-en`), `source`, `built` (date), `entries`; `frequency_source` when frequencies were added |
| `entry` | A headword and part of speech: "lit" (noun), with IPA |
| `sense` | The entry's English meanings, in order |
| `synonym` | Words in the same language with the same meaning |
| `form` | Inflected forms and their dictionary form: "lisait" → "lire", with grammar tags |
| `frequency`, `frequency_form` | Optional: dictionary forms by frequency rank, and the forms counted under each |

**Manifest** (`dictionaries.json`): `format`, then per dictionary `pair`, `language`, `url`, `built`, `download_bytes`, `installed_bytes`, `sha256`, and `frequencies` (the frequency source, if any). Book lists are listed in the same file under `catalogs`.

**The list in the app** (`Dictionaries`): fetched when Settings opens, when a tapped word has no dictionary, and for the library's cards. One fetch runs at a time and callers waiting for it share its result; a failed fetch is tried again after 1 and 3 seconds, and only then shows an error (with Try again in Settings). The last fetched list is saved (`dictionaries.json` in the app's cache) and shown at once on the next start while the fresh one loads, as the first request on a phone can take a long time.

**Install** (core `dictionary::install`): check the download's SHA-256 against the manifest, unpack it, open it and check its pair, then move it into place with an atomic rename. A file that fails any step is deleted, so a broken dictionary is never opened.

**Used for:**
- the word drawer (meanings, synonyms, IPA, dictionary forms, phrases);
- book counting: names, and forms counted with their dictionary form;
- the common form a word is saved in ("Perle" → "perle");
- the vocabulary check.

The French dictionary is about 11 MB to download and 43 MB on the phone.

## Word frequencies and the vocabulary check

### Frequency data

`dictgen` reads a frequency list through a **`FrequencySource`** (`crates/dictgen/src/frequency.rs`). Each source turns its own file into records: a word form, how common it is (per million words), and its dictionary form if the source knows it.

| Source | Languages | What it gives | Licence |
|---|---|---|---|
| `Lexique` (Lexique 3, New, Pallier et al.) | French | Forms with their dictionary forms; frequencies from books and film subtitles, averaged | CC BY-SA 4.0 |
| `FrequencyWords` (Hermit Dave, OpenSubtitles 2018) | 60+ | Forms only, from film subtitles; dictionary forms come from the dictionary's own "form of" links | CC BY-SA 4.0 |

`add_frequencies`:
- keeps only words the dictionary knows (which also drops names and typos);
- ranks dictionary forms by the summed frequency of their forms;
- writes `frequency` (up to 50,000 ranked dictionary forms) and `frequency_form` into the dictionary file, plus `frequency_source` in `meta`.

Whatever the source, the tables are the same, so the app never depends on the provider. A new provider is one more `FrequencySource`. In CI, French uses Lexique (downloaded from lexique.org) and every other language FrequencyWords' 50,000-word list. On the real data, French has 37,860 ranked dictionary forms with 125,374 word forms.

To add frequencies to an existing dictionary by hand:

```bash
cargo run --release -p allreads-dictgen -- --language fr --output fr-en.sqlite \
  --frequencies Lexique383.tsv --frequency-source lexique
```

### The check

Core `vocab_check::VocabCheck`. The app drives it as a `VocabCheckSession` one band at a time.

1. **Questions:**
   - About 6 dictionary forms from each of 11 frequency bands (top 250, 250–500, 500–1,000 … 16,000–24,000).
   - 10 made-up words mixed in. They are spliced from real words, use only letter sequences common in real words, are never one letter from a common word, and are checked against the dictionary.
   - Single letters and elided bits ("l", "l'un") are not asked.
2. **Stop:** after two bands in a row with fewer than half "yes", then 12 more words around where the reader's knowledge fades.
3. **Scoring:**
   - Each band's "yes" share is corrected for the "yes" given to made-up words.
   - It is made to fall with rarity (pool-adjacent-violators).
   - Ranks between bands are read off a log-scale line.
4. **Choices:** "safe" marks up to where the chance of knowing stays at least 95 %; "more" goes down to 85 %. Each shows its expected accuracy.
5. **Rules that keep marks right:**
   - Words answered "no" are never marked.
   - A "yes" beyond the cut-off counts only when it is at least 90 % likely to be real.
   - With more than 30 % "yes" on made-up words, or no made-up words, nothing is offered.
6. **Applying:** the chosen dictionary forms and all their forms are saved as known, with the source "Vocabulary check". Words already saved are left as they are. Known words get no Anki cards.
7. **After:**
   - **Undo** deletes the marked words nobody has touched since.
   - A marked word that leaves "known" counts as a correction and becomes the reader's own. Settings shows how many were set back.

On simulated readers with real French ranks, the safe choice is 98–99 % right in 40–90 questions; with 25 % guessing it is 97–98 % right.

**In the app:**
- A card in the library for new readers (next to the "Getting started" card with the DeepL key, the dictionary and the AnkiDroid deck), while the language's dictionary (installed or published) has frequencies.
- Settings → Dictionaries, with the counts and Undo.
- If the dictionary is not installed yet, the check downloads it first.

## Book difficulty ("% new")

Core `word_tally`.

- **What is counted:** each book's words, once, into `book_word`: how often each term key occurs, whether it is a name, and its dictionary form.
- **Names:** a word the dictionary knows only as a name, or that the book always capitalises inside sentences, is left out. The capitals rule is skipped for languages that capitalise nouns (German).
- **The share:** running words that are not saved (directly, or through a saved dictionary form) out of all running words. The library card, the reader's top bar and "Find a book" all use it.
- **Dots:** green up to 5 %, amber up to 12 %, red beyond.
- **The count stamp:** each book stores the parser and counting version and the dictionary it was counted with (`count_stamp`). A book is counted again when that changes, for example after a dictionary is installed.

## Find a book (book lists)

**Status:** the app side is in `main`. No book list is published yet: building the real French list (downloading and counting the books) is on the roadmap.

**File** (`catalogs/<language>.sqlite`, core `catalog`):

| Table | What |
|---|---|
| `info` | `format`, `language`, `count_stamp`, `built` |
| `word` | Every word once, numbered by how common it is across the list (0 = most common), with its dictionary form's number |
| `book` | Title, author, subjects, EPUB link, popularity, year, running words, and `counts`: the book's (word number, count) pairs, packed as varints (about 2 bytes a word) |

Every word of every book is kept exactly. 20 French novels take 1.4 MB (720 KB compressed), so 500 books come to about 12 MB.

**Build:**
- `cargo run -p allreads-dictgen --bin catalog -- --list books.jsonl --books epubs/ --dictionary fr-en.sqlite --language fr --output fr.sqlite` counts each EPUB with the app's own counting.
- Planned sources: French Wikisource (an OPDS feed of 9,335 CC BY-SA texts, 3,240 of them from 1900–1949), then Project Gutenberg. LibriVox and Open Library would add extra information.
- Fetching the books and the publishing workflow are still to do.

**In the app:**
- Listed in `dictionaries.json` under `catalogs` and installed like a dictionary: checksum, unpack, check the language, atomic rename.
- **Search:** title, author and subjects, ignoring case and accents.
- **Fit:** each book against the saved words, using the same numbers as the library card, plus "N to learn" (new words met five or more times).
- **Best fit order:** books at most 5 % new first, those with the most words to learn ahead; then the rest, easiest first.
- **Get:** downloads the EPUB and imports it.

## AnkiDroid

- **Connection:** AnkiDroid's API through its content provider, with its permission. It works only with AnkiDroid installed on the same phone.
- **Note types and deck:** one note type per language, "Allreads (Language)", found by name or created. Cards go into the chosen deck.
- **Outbox:** saving a learning word queues it (`anki_outbox`). The queue is sent after saves, when the app resumes, and when a deck is chosen. A failed push stays queued.
- **Forget:** forgetting a word suspends its card after a minute (the Undo window), via the `anki_suspend` queue, and tags it `allreads-forgotten`. AnkiDroid cannot un-suspend, so a word saved again gets a fresh card and the suspended one is deleted.
- **Import:** words come in from an AnkiDroid deck, an `.apkg` file, or a text/CSV export. The reader picks the fields. Status is one chosen level or follows Anki's review progress. Imported words show a badge and never get cards from Allreads.

## Translation

DeepL, with the reader's own API key (free plan: 500,000 characters a month). Translations are cached in the database per text, context and language pair, so the same sentence is never sent twice.
