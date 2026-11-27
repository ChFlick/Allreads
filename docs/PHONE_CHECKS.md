# Phone checks before v1.0

Things the JVM tests and screenshots can't show: taps, gestures, timing,
haptics, and the real AnkiDroid. Covers everything built since the phone
test of 2026-11-25. Install the latest pre-release APK from GitHub Releases
over the old one (keep the data), with AnkiDroid installed and a deck chosen.

Tick a box when it works; for a problem, write a short note under it.
About 20–30 minutes.

## 1. Start and look (2 min)

- [ ] **Splash:** cold start in dark mode shows the navy circle with the book, with no white flash. The same in light mode.
- [ ] **Icon:** the launcher icon's book fills most of the circle without touching its edge.
- [ ] **Old data:** books, positions and saved words from before the update are all still there. The database update (V8–V17) ran without a crash.

## 2. Library (3 min)

- [ ] **Continue reading** card shows the last book; Continue opens it at the saved place.
- [ ] Books without a cover get a drawn one (title on a colour); real covers still show.
- [ ] Each book shows a share of new words with a green/amber/red dot. A newly imported book gets its number after a moment.
- [ ] **Search pill:** typing filters by title and author; the sort chip and its menu (recently read / added / title / author) work, and the sort is remembered after restarting.
- [ ] **Import → Web article:** paste a news article's link. The book has the article's title and readable text, and accents are right.
- [ ] **Import → Paste text** makes a book titled from its first line.
- [ ] **Share → Allreads** from the browser imports the page.

## 3. Reader (6 min)

- [ ] **Opening a book** slides in from the side and Back slides it out. It feels smooth, also for a long book (no stutter before the text appears).
- [ ] Back in the library, the list is still scrolled where it was.
- [ ] **⋮ → Contents** lists the chapters with the current one marked; tapping another opens it at its start. For a single-chapter text the item is not there.
- [ ] **Top bar** slides away while reading down and comes back when scrolling up; the thin progress line stays and fills up to the chapter's end.
- [ ] **Double tap** on a learning word raises it one level with a short buzz, and the snackbar's Undo puts it back. A single tap still opens the drawer (after a short wait).
- [ ] **Long-press and drag** selects a phrase, with a buzz when the selection starts.
- [ ] **Chapter end card:** saved / raised / still new numbers look right; "Mark N as known" works and its Undo brings them back to new.
- [ ] **⋮ → Text and page:** font (Literata / Serif / Sans-serif), Sepia and Black pages, size and spacing change the page right away.
- [ ] On a tablet or in landscape, the text column stays narrow and centred.

## 4. Word drawer (4 min)

- [ ] Tap a word from the start of a sentence ("Perle"), save it: it is saved in lower case ("perle"), also on the Anki card.
- [ ] "✓ Saved as level 1" appears under the level bar.
- [ ] **Meanings in the book's language** (Settings → Translation, if turned on): the translation stays hidden until Reveal.
- [ ] **Look up in the web dictionary** is there and opens the browser. (Optional: with every browser disabled the button is gone instead of an error.)
- [ ] The play button speaks the word; "Speak words when tapped" speaks on tap.

## 5. Words tab (4 min)

- [ ] Switching tabs fades; each tab keeps its scroll position.
- [ ] The progress strip shows known words and "+N this week"; tapping it shows the chart.
- [ ] **Swipe left** raises a level (a tick when the swipe passes the threshold) with Undo.
- [ ] **Swipe right** asks "are you sure" and then forgets the word.
- [ ] **Undo after forgetting a word that has a card:** the word comes back with its level, meaning and sentence. In AnkiDroid the card is **not** suspended (also check again a few minutes later).
- [ ] **Undo after forgetting an imported word:** the "Anki · …" badge comes back.

## 6. AnkiDroid (6 min)

- [ ] **Forget → suspend:** forget a word that has a card and don't undo. After about a minute plus a save or an app resume, the card is suspended in AnkiDroid and the note has the tag `allreads-forgotten`.
- [ ] **Save it again:** saving the same word again deletes the suspended note and adds a fresh card (one card, not two).
- [ ] **Outbox (step 19):** uninstall AnkiDroid (or disable it), save two words, and check that Settings shows "2 words waiting". Reinstall AnkiDroid, grant access, choose the deck again, and check that both words arrive.
- [ ] **Import from AnkiDroid** (Words → ⋮ → Import from Anki → a deck): note types and fields are offered; "follow Anki's progress" gives sensible levels (well-reviewed cards known, new cards low). Check a few words against their intervals in AnkiDroid.
- [ ] After the import, the words show coloured in the reader, carry the "Anki · deck" badge, and no duplicate cards appear in AnkiDroid.
- [ ] **Import from a .apkg file** works the same way.

## 7. Settings (2 min)

- [ ] Overview: each row's summary is right (DeepL key, AnkiDroid connected / waiting, dictionaries, theme). Pages slide in and Back slides them out.
- [ ] Appearance: System / Light / Dark and wallpaper colours switch the app's colours at once.
- [ ] About: the licences open; the GitHub link opens the browser.
- [ ] **Interface text:** nothing shows up blank or as a raw key anywhere. (All text moved into resource files on 2026-11-27, so any empty label would be a bug.)

## Notes

<!-- Problems found, one per line, with the section number. -->
