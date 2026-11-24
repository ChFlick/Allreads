# PRD: reading-first language app

Nov 23, 2026 · Christoph Butz

## Problem, vision and goals

Reading real books is one of the best ways to learn a language, but looking up words breaks the flow, and the words you look up are rarely reviewed. The existing tools each miss something:

- Smart Book and LingQ are proprietary and charge for the good features.
- Lute, LWT and LinguaCafe are open source, but run on a desktop or a server.
- None of them sends looked-up words to Anki with their sentence.

**Vision:** an open-source Android reader that turns any book into a lesson. You tap what you don't know, read on, and review it later in Anki.

| Goal | Measure |
| --- | --- |
| G1. Read a real EPUB with one-tap lookups | Tap to translation in under 100 ms (dictionary) or 1 s (online) |
| G2. Every saved word becomes an Anki card with its context | Saved terms show up in AnkiDroid without manual steps |
| G3. See progress | Known-word count and % of a chapter known, per book |
| G4. Private and offline by default | Reading and saving work in flight mode; no account, no telemetry |

## Users and user stories

**Primary user:** an adult self-learner at A2–B2, who reads on their phone and already uses Anki. For example, a German speaker learning French with classic novels. **Secondary users:** beginners who lean on parallel translation, and advanced readers who only need help with rare words and idioms.

| ID | As a reader, I want to… | So that… | Priority |
| --- | --- | --- | --- |
| US1 | import an EPUB or TXT from my phone | I can read what I want | P0 |
| US2 | see new, learning and known words in different colours | I know where to focus | P0 |
| US3 | tap a word and see its meaning in a sheet | I can read on without leaving the book | P0 |
| US4 | save a word with a status and translation | it is tracked and highlighted everywhere | P0 |
| US5 | have saved words appear in AnkiDroid with the sentence | I review them without extra work | P0 |
| US6 | pick up where I stopped | reading feels like a normal e-reader | P0 |
| US7 | mark all remaining new words on a page as known | my vocabulary count stays honest | P1 |
| US8 | select several words as a phrase | idioms and fixed phrases are saved as one term | P1 |
| US9 | translate the current sentence, or show translations under each sentence | I can follow hard passages | P1 |
| US10 | ask the AI why a word or form is used here | I understand grammar in context | P1 |
| US11 | hear a sentence or word read aloud | I learn pronunciation | P1 |
| US12 | browse and download free public-domain books in my language | I never run out of material | P2 |
| US13 | export my vocabulary as `.apkg` or CSV | I'm not locked in | P2 |

## Functional requirements

P0 is needed for the MVP (M1–M2), P1 for v1.0, P2 is nice to have.

| ID | Area | Requirement | Priority | Milestone |
| --- | --- | --- | --- | --- |
| F1 | Library | Import EPUB 2/3 and UTF-8 TXT through the system file picker, and copy the file into app storage | P0 | M1 |
| F2 | Library | List books with title, author, cover, language and progress; delete a book | P0 | M1 |
| F3 | Library | Set the book's language on import (suggested from EPUB metadata) | P0 | M1 |
| F4 | Reader | Render a chapter as paragraphs in a vertical scroll, with headings and italics kept | P0 | M1 |
| F5 | Reader | Colour each word by status (new, learning 1–5, known, ignored), with a legend and a colour-blind-safe theme | P0 | M1 |
| F6 | Reader | Chapter navigation (table of contents, next and previous) and an exact resume position | P0 | M1 |
| F7 | Reader | Font size, line spacing, light and dark theme | P1 | M1 |
| F8 | Reader | "Mark rest of chapter as known" | P1 | M1 |
| F9 | Reader | Long-press and drag to select a multi-word phrase | P1 | M3 |
| F10 | Lookup | Word sheet: the word, the sentence it's in, its saved translation if any, and a status picker | P0 | M1 |
| F11 | Lookup | Machine translation of the word (in context) through the configured translator | P0 | M1 |
| F12 | Lookup | Offline dictionary entry with lemma and part of speech, from downloaded Wiktionary data | P1 | M3 |
| F13 | Lookup | Translate the sentence; parallel mode shows a translation under every sentence | P1 | M3 |
| F14 | Lookup | "Explain" with the user's own AI key: meaning in context, grammar, why this form; answers cached | P1 | M3 |
| F15 | Vocabulary | Save, edit and delete terms; statuses 1–5, 98 (ignored), 99 (well known) | P0 | M1 |
| F16 | Vocabulary | Link inflected forms to a lemma (parent term) | P1 | M3 |
| F17 | Vocabulary | Word list with search, filter by status and language | P1 | M2 |
| F18 | Anki | Detect AnkiDroid, request its permission, and pick or create a deck | P0 | M2 |
| F19 | Anki | Create the app's note type on first use: Word, Lemma, Translation, Sentence (word bolded), Sentence translation, Source | P0 | M2 |
| F20 | Anki | Push a note when a term is saved with status 1–4; update the same note when the term is edited; no duplicates | P0 | M2 |
| F21 | Anki | Queue pushes when AnkiDroid is unavailable and retry later | P1 | M2 |
| F22 | Anki | Export terms as `.apkg` and CSV | P2 | M2 |
| F23 | Audio | Read the current sentence or chapter aloud, highlighting the sentence being read; speak a word from the sheet | P1 | M4 |
| F24 | Settings | Languages (from presets), translator, AI provider and key, Anki deck, and reading preferences | P0 | M1 |
| F25 | Stats | Known words per language, new terms per day, % known per chapter | P1 | M5 |
| F26 | Discover | Browse and download from Project Gutenberg and Standard Ebooks by language | P2 | M5 |
| F27 | Data | Back up and restore the database and books as one file | P1 | M5 |

## Non-functional requirements

| Area | Requirement |
| --- | --- |
| Offline | Importing, reading, saving words and the Anki push work with no network; only translation, AI and downloads need it |
| Performance | Open a 10,000-word chapter in under 300 ms on a mid-range phone; respond to a tap in under 100 ms; scroll at 60 fps |
| Size | APK under 30 MB, not counting dictionaries, which are optional downloads |
| Privacy | No accounts, analytics or telemetry. Text goes only to providers the user configures, and keys are stored in the Android Keystore |
| Openness | Every dependency open source, so the app qualifies for F-Droid; no Google Play Services |
| Platform | Android 8.0+ (API 26), arm64, armv7 and x86_64 |
| Accessibility | Works with TalkBack and system font scaling; status colours aren't the only cue |
| Data | One SQLite file plus book files; schema migrations never lose data |
| Quality | Core logic covered by Rust unit and snapshot tests; CI runs `cargo test`, `clippy` and a Gradle build on every push |

## Out of scope, success measures and open questions

**Out of scope for v1:** a built-in review scheduler (Anki does it), iOS, accounts and sync, social features, video and subtitles, a bundled catalogue of copyrighted books, and paid AI hosted by us.

**Success measures (first six months):**

- You read at least one full French novel in the app.
- At least 500 cards reach Anki through the app, with no manual fixes.
- Looking up and saving a word takes at most 2 taps.
- No crashes or data loss in daily use; the app is on F-Droid.

**Open questions:**

- [ ] Note type: basic cards (word → meaning), sentence cards with the word bolded, or both as card templates?
- [ ] Which dictionary source per language pair (French → English, French → German), and how big is it after filtering?
- [ ] Should statuses sync back from Anki (for example, mark a word known once its card is mature)?
- [ ] Name and licence.
