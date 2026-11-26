# Polish list

Paper cuts and polish ideas from a run through every screen (2026-11-25), done by reading the UI code and the Paparazzi screenshots. There's no emulator in the cloud session, so items marked **(phone)** need checking on a device. Size: S = under an hour, M = a few hours, L = a day or more.

## Must-have before v1.0

| # | Where | Paper cut | Fix | Size |
|---|---|---|---|---|
| 1 | App | **No app icon.** There is no `mipmap` folder, so the launcher shows the Android default. | An adaptive icon (foreground and background vectors, plus a monochrome layer for themed icons). | S–M |
| 2 | Library, Words | Languages show as codes ("FR") in the book row, the import dialog and the Words chips. | Show the name in the phone's language (`Locale.displayLanguage`), as Settings already does. | S |
| 3 | Library | Deleting a book is only possible with an undiscoverable long-press. | A ⋮ menu on each card with "Delete". Keep the long-press too. | S |
| 4 | Library | The "Import book" button covers the last card. | Bottom content padding of about 88 dp. | S |
| 5 | Word drawer | When the chosen status is New, the main button says "Forget", even for a word that was never saved. | Show "Forget" only when `word.saved != null`. Otherwise disable the button or keep it as "Save". | S |
| 6 | Word drawer | Closing the drawer by swiping or tapping outside silently throws away a typed translation or notes. | Either save on close when something changed, or ask "Discard changes?". Autosave fits better with a reading flow. | S–M |
| 7 | Reader | "Mark the rest of this chapter as known" can't be undone, and one wrong tap marks hundreds of words. | An "Undo" action on the snackbar. Core: return the term ids it created, then delete them on undo. | M |
| 8 | Settings | "Remove" for the DeepL key has no confirmation. | A confirm dialog. | S |
| 9 | Settings, About | Wiktionary data is CC BY-SA. Attribution is there, but no licence screen lists the bundled libraries. | An "Open-source licences" entry (the AboutLibraries plugin, or a hand-written list) and a link to the GitHub repository. | S–M |

## Reader

| # | Paper cut | Fix | Size |
|---|---|---|---|
| 10 | Every sentence row has two buttons (▶ and 文A), which takes width (asymmetric 20/8 dp margins) and adds a lot of visual noise to a page of prose. | Option A: one small ⋯ per row that opens "Read aloud / Translate". Option B: hide the buttons and show them on a tap of the sentence's gutter. Worth trying both on the phone. | M |
| 11 | "Stop reading" uses the ✕ (Close) icon. | A proper stop icon (the extended icons have `Stop`; or a small vector). | S |
| 12 | No sense of where you are in a chapter or in the book. | A thin progress bar under the top bar (position in the chapter), and "chapter 3 of 12" in the subtitle. | S |
| 13 | Chapters can only be changed with the buttons at the very bottom. | A table of contents from the top-bar menu (the core already has chapter titles). | M |
| 14 | The screen goes to sleep while reading. | `FLAG_KEEP_SCREEN_ON` while the reader is open, perhaps as a setting. | S |
| 15 | A sentence's translation error has no retry. | A "Retry" button, like the drawer has. | S |
| 16 | Long-press and drag for phrases is invisible. | A one-time hint ("Long-press and drag to select a phrase") the first time the drawer opens. | S |
| 17 | No haptic feedback when a long-press starts a selection. | `LocalHapticFeedback.performHapticFeedback(LongPress)`. | S |
| 18 | On tablets and in landscape the lines get very long. | Cap the text column at about 640 dp and centre it. | S |
| 19 | **(phone)** Does opening a big chapter show anything while it loads? | Check, then add a progress indicator if it's blank. | S |
| 20 | Chapter title and book title compete in the top bar. | Book title as the small line, chapter as the main line (done already). Consider hiding the bar when scrolling down. | S |

## Word drawer

| # | Paper cut | Fix | Size |
|---|---|---|---|
| 21 | Save sits at the top right, but the status chips you pick before saving are further down. | Keep Save at the top, and make a chip tap save straight away (LingQ style). It's a big speed win for reading. Or move Save under the chips. | S–M |
| 22 | The word's ▶ only changes colour while speaking. | Change the icon to stop while speaking, as in the reader. | S |
| 23 | Phrase suggestions say "Phrase: …" as plain button text. | A small "Phrase" label, then the phrase in bold and the gloss below it. | S |
| 24 | "No browser installed" uses a Toast, while everything else uses snackbars. | A snackbar, or hide the button when no app can open it. | S |
| 25 | The web dictionary link is only visible once the drawer is pulled up. | An icon button next to ▶ in the top row. | S |

## Words

| # | Paper cut | Fix | Size |
|---|---|---|---|
| 26 | The editor can't change the lemma, has no sentence context, no ▶, and forgetting is hidden behind picking "New". | Reuse the drawer's layout: sentence, lemma, ▶, and an explicit "Forget word" button. | M |
| 27 | The header (counts, search, filters) doesn't scroll, so a small phone has little room for the list. | Put the header in the LazyColumn, or collapse it on scroll. | S |
| 28 | No sorting. | Sort by recently saved, alphabetical or status. | S–M |
| 29 | Anki status is invisible per word. Words that failed to go to AnkiDroid are only a count in Settings. | A small "not in Anki yet" mark, plus a "Waiting" filter. | M |

## Settings

| # | Paper cut | Fix | Size |
|---|---|---|---|
| 30 | One long page with seven sections. | Group them (Translation & dictionaries / AnkiDroid / Reading & audio / About), as sub-pages or collapsible sections. | M |
| 31 | "deepl.com/pro-api" is plain text. | Make it a link. | S |
| 32 | The theme chips use the enum names. | Proper labels ("System", "Light", "Dark"), checking the casing. | S |
| 33 | The preview sentence is always French. | Show a sentence in the language of the last opened book. | S |

## App-wide

| # | Paper cut | Fix | Size |
|---|---|---|---|
| 34 | No first-run guidance: a new user doesn't know they need a DeepL key and the dictionary. | A short welcome card in the empty library with the three set-up steps (import a book, DeepL key, AnkiDroid deck), each linking to its place. | M |
| 35 | Back from Words or Settings closes the app instead of going to Library. | A BackHandler that returns to the Library tab first. | S |
| 36 | The tab icons are generic (Home, List), and `Icons.Default.List` is deprecated. | Book and translate-style icons (`AutoMirrored.List` at least). | S |
| 37 | All strings are hard-coded in Kotlin, so the UI can't be translated. | Move them to `strings.xml`. Not urgent, but every new screen adds to the work. | L |
| 38 | No splash screen, so there's a white flash in dark mode **(phone)**. | `core-splashscreen` with the theme colours. | S |
| 39 | Predictive back and animations between Library and the reader. | `AnimatedContent`, or Navigation Compose later. | M |
| 40 | Accessibility: words in the reader can't be reached with TalkBack. | Long-term: per-sentence semantics with custom actions. At least label every icon button (mostly done). | L |

## Done

2026-11-25, quick-fix batch: 1 (app icon), 2, 3, 4, 5, 8, 11, 14, 15, 31, 35, 36. Item 32 needed no change, because the enum names already read "System / Light / Dark". Items 5, 6 and 21 are covered by chip-saves (see DECISIONS, 2026-11-26), and item 10 stays as it is. Item 24 is postponed: hiding the button needs a `<queries>` entry for browsers, and the Toast does the job.

## Suggested order

1. **Quick fixes (one sitting):** 1, 2, 3, 4, 5, 8, 11, 14, 15, 24, 31, 32, 35, 36.
2. **Reading flow:** 6, 21, 7, 10, 12, 16, 17.
3. **Before announcing v1.0:** 9, 34, 38.
4. **Later:** 13, 26–30, 37, 39, 40.
