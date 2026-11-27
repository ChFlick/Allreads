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

1. ~~Quick fixes~~ (done).
2. **Reading flow:** 7, 12, 16, 17.
3. **Before announcing v1.0:** 9, 34, 38.
4. **Later:** 13, 26–30, 37, 39, 40.

---

# Round 2: design and improvements (2026-11-26)

A second pass, looking at the app as a designer and a learner would rather than hunting bugs. Based on the current screenshots and code. Numbers continue from the first list.

## Look and feel

| # | Where | Observation | Suggestion | Size |
|---|---|---|---|---|
| 41 | App | The app's colours don't match the icon. The screenshots show Material's default purple, while the icon is navy and amber. With wallpaper colours on, the app looks different on every phone. | Give the app its own colour scheme from the icon (navy primary, amber accent; light and dark). Keep wallpaper colours as an opt-in in Appearance. | S–M |
| 42 | Reader | Nearly every word has an underline (new words), so a page of a new book looks like a page of links. The underlines draw the eye more than the coloured learning words do. | Lighter by default: a dotted line or a very faint blue tint. Or a setting "Mark new words: underline / tint / nothing". LingQ uses a pale blue fill. | S |
| 43 | Reader | Learning levels 2–4 are hard to tell apart (three similar yellows), and 4–5 nearly disappear on the light background. | One hue that gets fainter as you learn the word: level 1 strong, level 5 barely there. The page then shows "how much is left" at a glance. Check contrast in both themes. | S |
| 44 | Reader | ▶ is a bare icon, but 文A is a filled circle, so the two sentence buttons look unrelated, and the filled circles make a strong column down the right side. | Same style for both: bare icons, or one small tonal pill holding both. (The buttons themselves stay, as decided.) | S |
| 45 | Reader | A sentence's translation is grey serif text, almost as big as the original, so it blends into the next sentence. | Show it in a sans-serif font, smaller, with a thin coloured line on its left, like a quote. | S |
| 46 | Reader | Each sentence is its own row, so paragraphs read as a stack of separate sentences. The gap between paragraphs (14 dp) and the gap between sentences (2 dp) are close to each other. | Make paragraph gaps clearly bigger (24 dp) and sentence gaps 0, so paragraphs read as blocks again. | S |
| 47 | Reader | The reading font is the phone's default serif. | Bundle **Literata** (OFL, made for long reading in Google Play Books) and add a font choice (Literata / system serif / sans). | S–M |
| 48 | Reader | Only light and dark. | Add a **sepia** reading theme and a true-black option for OLED. These are standard in readers and easy to do with the reader's own colours. | M |
| 49 | Library | Books without a cover show an empty grey box. | A generated cover: the title on a colour picked from the title, like Apple Books does. | S |
| 50 | Library | Progress is only "chapter 4 of 12". | A thin progress bar on each card, and the last-read book first. | S |
| 51 | Library | The most common action, "carry on reading", is as small as everything else. | A "Continue reading" card at the top with the last book, its chapter and a big Continue button. | S–M |
| 52 | Words | The trailing "2", "4", "1" are cryptic, and in dark mode the coloured dots are muddy and hard to tell apart. | A small pill in the status colour with "Level 2" or "Known", the same colours as in the reader. | S |
| 53 | Words | Words saved at the start of a sentence keep their capital letter ("Coq", "Perle", "Pierre précieuse"). The cards in Anki get it too. | Save the display form in lower case when the capital only comes from the sentence start, using the same rule as the DeepL lowercasing. Proper nouns stay as they are. | S–M |
| 54 | Words | The header (language chip, counts, search, filters) takes a third of the screen. The language chip shows even when there's only one language. | Hide the language row for a single language. Turn the counts into the screen's subtitle. Use a compact search field. | S |
| 55 | Drawer | The status chips are plain grey, so nothing teaches you which colour means which level. | Fill each chip with its status colour (the selected one gets an outline or a tick). | S |
| 56 | Drawer | After tapping a chip, the only sign that the word was saved is the highlight. | A brief "Saved · in AnkiDroid" line under the chips, or a check mark on the chip. | S |
| 57 | Settings | No screenshot exists yet, so this screen hasn't had a visual review. It's the longest screen. | Add Settings to the Paparazzi screenshots, then review it like the others (ties in with #30). | S |

## Features that would make the app better for learning

| # | Idea | Why | Size |
|---|---|---|---|
| 58 | **Difficulty of a chapter or book**: "18 % new words" on the library card and the chapter start. | Helps you pick a book at the right level (the 95–98 % known-words rule). The core already knows every word's status. | M |
| 59 | **Statistics**: known words over time, words saved this week, pages read. A small card at the top of Words. | Motivation; LingQ's "known words" count is its main progress signal. Needs a daily snapshot table in the core. | M |
| 60 | **More dictionaries**: German, Spanish, Italian. | dictgen already handles any kaikki.org language; it's a run of the Dictionaries workflow plus language presets. Big value for little code. | S–M |
| 61 | **Import from clipboard or a web page** as a book. | News and blog articles are good reading material, often better than books for beginners. | M |
| 62 | **Text settings in the reader**: an "Aa" button in the top bar with size, spacing, font and theme. | Changing these in Settings means leaving the book. | M |
| 63 | **Search in the book** and a **table of contents** (#13). | Finding a passage again. | M |
| 64 | **Tap-to-level-up while reading**: a second tap on a learning word raises its level by one without opening the drawer. | Revisiting words as you meet them again is how levels are meant to move. Needs a clear gesture so it doesn't happen by accident. | S–M |

## Done (round 2)

2026-11-26: 41 (own navy/amber colours; wallpaper colours are an opt-in switch in Appearance), 42 (dotted underline), 43 (one amber hue fading by level), 44, 45, 46, 52, 55 (chips, also in the Words editor). More dictionaries (60) are on hold for now.

2026-11-26: 49 (covers drawn from the title and author for books without one), 50 (progress bar by chapter; recently read books first, via `book.last_read_at`, migration V8), 51 (Continue reading card).

## Suggested order (round 2)

1. **Design quick wins:** 41, 42, 43, 44, 45, 46, 52, 55. Together these change the whole look, and each is small.
2. **Library:** 49, 50, 51.
3. **Reading comfort:** 47, 48, 62.
4. **Learning features:** 58, 60, then 59, 64.

2026-11-26: library search, filters and sort (not on the original list). The search icon opens a field for title or author, with chips for All / Reading / Not started and, when there are several, the books' languages. The sort menu offers recently read, recently added, title and author, and remembers the choice. The Continue card only shows with the default view.
Redesigned the same day: a "Search your library" pill always at the top (in place of the title bar), and one row of chips under it: the sort chip with its menu, then Reading / Not started and the languages as on-off filters.

2026-11-26: 7 (Undo on the "Marked N words as known" snackbar; the core returns the added term ids and `undo_mark_known` deletes those still marked known), 12 (a thin progress bar under the reader's top bar, by the last visible sentence) and 16 (a one-time tip in the word drawer about long-press and drag; gone after "Got it" or the first dragged phrase).

2026-11-26: 47 (Literata bundled as variable fonts, ~1.9 MB; Font setting Literata / Serif / Sans-serif), 48 (Sepia page setting for the reader, light in either app theme; true black not done), 53 (words are saved in their common lower-case form when the dictionary knows one, the same rule as for DeepL), 54 (Words header: search pill and one chip row with counts; language chips only for several book languages), 56 ("✓ Saved at level 2" under the chips), 57 (Settings screenshot: `SettingsScreenshot`, one tall image with fakes). Settings review: "Translate into" is a dropdown, sliders have a grey track, the reading preview shows font and page colours, About is split into short lines with a GitHub link. Also fixed: Words started in the language of the oldest book since the library order changed; it now uses the most recently read one, and only offers languages that have books.

2026-11-26, learning features: 58 (share of new words per book on the library cards, with a green/amber/red dot at 5 % and 12 %, and per chapter in the reader's top bar; the core counts a book's words once into `book_word`, migration V9), 59 (Words: known words with "+N this week" and a 90-day chart of known and learning words, from daily counts in `word_count_day`), 61 (Import → Web article or Paste text, and "Share → Allreads" from any app; the core extracts the article from the page's HTML), 64 (double-tap a learning word or phrase to raise it one level, with Undo; a setting, on by default, because single taps then wait for a possible second tap). Also fixed: TXT books were titled with their copy's random file name.

2026-11-26: the difficulty leaves out names and counts word forms with their dictionary form. A name is a word the offline dictionary knows only as a name, or one it does not know that the book always capitalises (at least once inside a sentence). Names are not underlined in the reader. A form without meanings of its own ("lisait") counts as known once its dictionary form ("lire") is saved, in the book's and the chapter's share; the reader still colours each spelling on its own (word families are on the roadmap). Books are counted again after installing or removing a dictionary (migration V10).

2026-11-26, code review of the day's work: fixed a shared link being imported again after rotation; undo of "mark the rest as known" deleting words edited since; web pages in Latin-1 importing with broken accents; http links that redirect to https failing; links ending in ")" being cut; books read before the update showing as not started (migration V11); overlapping difficulty counts; counts racing a dictionary install; Words losing languages whose books were deleted.

2026-11-26, clean-up pass (reuse, simplification, efficiency, layering) over the day's work. Done: unused imports removed; shared `SwitchSetting`, `ChoiceChips`, `DotLabel`, `BookProgressBar` and an undo-snackbar helper; settings enums parsed by one helper and the library sort stored as `BookSort`; `MarkedKnown` carries only the ids; a double tap now calls a new core `set_term_status` (one call, only the status changes) instead of find-then-save; the library loads every counted difficulty in one query and counts the rest in a single background job; covers are decoded at the size shown and cached; the progress counts are written inside the save's transaction for one language, with an index (migration V12); the Words tab gets its languages from one core query; chip counts and the library's filtered list are worked out once per state.

Left for later (tech debt, moving logic from Kotlin into the core as the project rules ask): all done, see below.

2026-11-26: moved into the core: the common form of saved words (`dictionary::common_form`, sharing `WordKinds::is_common` with the name rule); book counting (`word_tally::count_book`, with a stamp of rules and dictionary per book instead of a change counter, migration V13; the reader counts a new book on first open); and writing pasted text and articles as TXT books (`prepare_text`, `prepare_article`; picked files are copied under their own name, so `import_book` lost its title override).

2026-11-26: the rest of the tech debt: web pages are decoded in the core (`article::decode_html`, with `encoding_rs`: byte-order mark, then the HTTP header, then `<meta charset>`, else UTF-8; Kotlin passes the raw bytes); the progress chart's daily series is built in the core (`progress_series`, with its own date arithmetic, so no date library); core tests share `new_term` and `insert_book` from `db::tests_support`.

2026-11-27, design review (artifact "Allreads Design Review"), all changes built: word drawer leads with word, pronunciation and meaning, then one-row `LevelBar`, quiet phrase rows, the sentence with the word marked, notes and dictionary form as edit-in-place rows; reader with rounded highlight marks, a chapter-end card (saved, raised, still new, "Mark N as known", next chapter), the difficulty dot in the top bar and an inset progress line; library as plain rows with one meta line, covers with a shadow, a language badge only outside the main language, "＋ Import" and a first-run "Start reading" page (items 34, 49); Words with a one-line progress strip (tap for the full chart), level edges, dictionary forms in rows, swipe left to raise a level and right to forget, both with Undo; Settings as an overview of seven pages with summaries (item 30), the "Text and page" page also as a sheet in the reader (item 62), and licences on About (item 9); Literata for display text, three corner sizes.

2026-11-27: 38 (no white flash at start: the window and the system splash use the app's colours, light and dark, with the icon on its navy circle, through `core-splashscreen`), 17 (haptics: a long-press when a phrase selection starts, a confirm on a double-tap raise, a tick when a Words swipe passes its threshold), 18 (the reader's text column is at most 640 dp wide and centred), 48 (a Black page for OLED screens, next to Sepia), and the reader's top bar slides away while reading down and returns on the way up (the progress line stays; the system bars stay too, to avoid the layout jumping).

2026-11-27: 13 and 63 (half: a table of contents from the reader's ⋮ menu; untitled chapters show as "Chapter N"; search in the book is still open), 37 (every user-facing text is in `strings.xml`; `AppText` reads the resources outside composables, and brand names and the voice samples are not translatable), 39 (the reader and the settings pages slide in and back out, the tabs fade through, the Anki import rises over the list, and the tabs keep their scroll positions while a book is open; predictive back is still open) and 24 (the web dictionary button is hidden when no browser can open the link, which replaces the last Toast). 40 (TalkBack) is planned on the roadmap.

2026-11-27: undoing a forget in Words now brings the whole word back (sentence, import badge, AnkiDroid card and queue place; the core keeps a copy for a day, migration V17), and a queued card suspension is no longer dropped when AnkiDroid fails to answer for a moment. Phone checklist for v1.0: `PHONE_CHECKS.md`.

2026-11-27, app check after Find a book: every screen rendered in light and dark, plus Android lint (now run in CI). Fixed: counts read "1 books" or "1 words" in seven places (now real plurals, with `AppText.plural`); the reader shows a spinner while a big chapter opens instead of a blank page (19); the level bar told TalkBack ", selected" in English (now the selected state itself, read in the phone's language); "Read aloud" in Settings said "1.0× · speak on tap off" (now "Speed 1.0×", or "Speed 1.0× · words spoken on tap"); Back in Find a book clears a search first, as in the library; the French preview sentence is no longer offered for translation; two duplicate strings merged; the reader's and Find a book's `open` return whether a new opening started (lint's "remember must not return Unit"). Lint ignores the UniFFI bindings' Android 13 cleaner (checked at run time) and two AnkiDroid-only rules that come with its API library. Already done before, now noted: 22 (the drawer's ▶ turns into stop). Still open: 25, 27–29, 33 (the preview sentence is still French; it should follow the last book's language).
