//! `App`: the library object Kotlin creates once, and the calls it offers.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

use allreads_core::db::book::Book as BookRow;
use allreads_core::db::term::NewTerm;
use allreads_core::db::translation_cache::CacheKey;
use allreads_core::db::Database;
use allreads_core::dictionary::{self, common_form, Dictionary, DictionaryError};
use allreads_core::library::{self, OpenedBook};
use allreads_core::presets::builtin_presets;
use allreads_core::reader::{parse_blocks, ParsedChapter};
use allreads_core::text::phrases::{find_phrases, WordSlot};
use allreads_core::text::{LanguageRules, PARSER_VERSION};
use allreads_core::translate::deepl::DeepL;
use allreads_core::translate::{TranslateError, TranslationRequest, Translator};
use allreads_core::word_tally;

use crate::error::{poisoned, AllreadsError};
use crate::records::{
    saturate_u32, AnkiNote, AnkiNoteType, AvailableDictionary, BookDifficulty, BookDifficultyOf,
    BookMeta, BookSummary, Chapter, ChapterBlock, DeepLKeyStatus, DictionaryLookup, ImportGroup,
    ImportRow, ImportStatus, ImportSummary, InstalledDictionary, LanguageInfo, MarkedKnown,
    PhraseMark, PhraseSuggestion, PreparedText, SavedTerm, TermInput, Token, Translation,
    WordProgress, WordStatus,
};

/// How many opened books `App` keeps in memory.
const OPEN_BOOKS: usize = 3;

/// The app's library: one `SQLite` database and the books opened from it.
///
/// Create it once and keep it for the life of the app. Kotlin calls it on
/// `Dispatchers.IO`, so shared state sits behind a `Mutex`.
#[derive(uniffi::Object)]
pub struct App {
    db: Mutex<Database>,
    data_dir: PathBuf,
    /// Recently opened books, most recent last, so reading a chapter does not
    /// re-parse the file. Capped at [`OPEN_BOOKS`] to bound memory and file handles.
    open_books: Mutex<Vec<(i64, Arc<OpenedBook>)>>,
    /// Installed dictionaries opened so far, by pair (e.g. `fr-en`).
    dictionaries: Mutex<HashMap<String, Dictionary>>,
}

impl App {
    /// A stored id setting; an unreadable value counts as unset.
    fn setting_id(&self, key: &str) -> Result<Option<i64>, AllreadsError> {
        Ok(self
            .db()?
            .setting(key)?
            .and_then(|value| value.parse().ok()))
    }

    fn db(&self) -> Result<MutexGuard<'_, Database>, AllreadsError> {
        self.db.lock().map_err(|_| poisoned())
    }

    /// Runs `f` on the language's `-en` dictionary, opening it on first use.
    /// `None` if no dictionary is installed for the language.
    fn with_dictionary<T>(
        &self,
        language_code: &str,
        f: impl FnOnce(&Dictionary) -> Result<T, dictionary::DictionaryError>,
    ) -> Result<Option<T>, AllreadsError> {
        let pair = format!("{}-en", language_code.to_lowercase());
        if !dictionary::valid_pair(&pair) {
            return Ok(None);
        }
        let mut open = self.dictionaries.lock().map_err(|_| poisoned())?;
        if !open.contains_key(&pair) {
            let path = self.dictionaries_dir().join(format!("{pair}.sqlite"));
            if !path.is_file() {
                return Ok(None);
            }
            open.insert(pair.clone(), Dictionary::open(&path)?);
        }
        Ok(Some(f(&open[&pair])?))
    }

    fn dictionaries_dir(&self) -> PathBuf {
        self.data_dir.join("dictionaries")
    }

    /// The articles stripped from imported words in a language ("le|la|les|l'").
    fn articles(&self, language_code: &str) -> Result<String, AllreadsError> {
        Ok(self
            .db()?
            .language(language_code)?
            .ok_or_else(|| AllreadsError::Missing {
                what: format!("language {language_code}"),
            })?
            .articles)
    }

    fn books_dir(&self) -> PathBuf {
        self.data_dir.join("books")
    }

    /// Writes paragraphs as a TXT book in the books folder.
    fn write_text(
        &self,
        title: String,
        language: Option<String>,
        paragraphs: &[String],
    ) -> Result<PreparedText, AllreadsError> {
        let path =
            library::write_text_book(&self.books_dir(), &title, paragraphs).map_err(|e| {
                AllreadsError::Storage {
                    reason: e.to_string(),
                }
            })?;
        // The title as the file name keeps it (reserved characters removed).
        let title = path
            .file_stem()
            .map_or(title, |stem| stem.to_string_lossy().into_owned());
        Ok(PreparedText {
            path: path.display().to_string(),
            title,
            language,
        })
    }

    fn covers_dir(&self) -> PathBuf {
        self.data_dir.join("covers")
    }

    /// The opened book for `book_id`, opening its file on first use.
    fn opened_book(&self, book_id: i64) -> Result<Arc<OpenedBook>, AllreadsError> {
        {
            let mut books = self.open_books.lock().map_err(|_| poisoned())?;
            if let Some(position) = books.iter().position(|(id, _)| *id == book_id) {
                // Move it to the end: it is now the most recently used.
                let entry = books.remove(position);
                let book = Arc::clone(&entry.1);
                books.push(entry);
                return Ok(book);
            }
        }
        let row = self.book_row(book_id)?;
        // Parse without holding any lock; another thread may race us, which is harmless.
        let book = Arc::new(OpenedBook::open(Path::new(&row.file_path))?);
        self.repair_chapter_list(&row, &book)?;
        self.remember_open_book(book_id, Arc::clone(&book))?;
        Ok(book)
    }

    /// Brings the stored chapter list in line with the book file, e.g. for a
    /// book imported before empty chapters were skipped: titles are rewritten,
    /// cached chapters dropped, and a position past the end reset.
    fn repair_chapter_list(&self, row: &BookRow, book: &OpenedBook) -> Result<(), AllreadsError> {
        let count = book.meta().chapters;
        let db = self.db()?;
        if db.chapter_titles(row.id)?.len() == count {
            return Ok(());
        }
        db.set_chapter_titles(row.id, &book.chapter_titles()?)?;
        db.clear_chapter_cache(row.id)?;
        if row.chapter_index as usize >= count {
            db.save_position(row.id, 0, 0)?;
        }
        Ok(())
    }

    /// Adds a book to the recently opened list, dropping the oldest past [`OPEN_BOOKS`].
    fn remember_open_book(&self, book_id: i64, book: Arc<OpenedBook>) -> Result<(), AllreadsError> {
        let mut books = self.open_books.lock().map_err(|_| poisoned())?;
        books.retain(|(id, _)| *id != book_id);
        books.push((book_id, book));
        if books.len() > OPEN_BOOKS {
            books.remove(0);
        }
        Ok(())
    }

    /// The tokenizing rules of a stored language.
    fn rules(&self, language_code: &str) -> Result<LanguageRules, AllreadsError> {
        let language =
            self.db()?
                .language(language_code)?
                .ok_or_else(|| AllreadsError::Missing {
                    what: format!("language {language_code}"),
                })?;
        LanguageRules::new(&language).map_err(|reason| AllreadsError::Database { reason })
    }

    /// Today's counting stamp for a language: the counting rules and the
    /// installed dictionary's pair and build date, if any.
    fn count_stamp(&self, language_code: &str) -> Result<String, AllreadsError> {
        let dictionary = self.with_dictionary(language_code, |d| d.meta("built"))?;
        let identity = dictionary.map(|built| {
            format!(
                "{}-en {}",
                language_code.to_lowercase(),
                built.unwrap_or_default()
            )
        });
        Ok(word_tally::count_stamp(identity.as_deref()))
    }

    /// A chapter's tokens from the cache, or parsed and cached on a miss.
    fn parsed_chapter(&self, row: &BookRow, index: u32) -> Result<ParsedChapter, AllreadsError> {
        let cached = self.db()?.cached_chapter(row.id, index, PARSER_VERSION)?;
        if let Some(chapter) = cached.as_deref().and_then(ParsedChapter::from_bytes) {
            return Ok(chapter);
        }
        // Parse without holding the database lock.
        let rules = self.rules(&row.language_code)?;
        let blocks = self.opened_book(row.id)?.chapter_blocks(index as usize)?;
        let chapter = parse_blocks(&blocks, &rules);
        // The cache only saves time: if writing it fails, the chapter still opens.
        let _ = self
            .db()?
            .store_chapter_cache(row.id, index, PARSER_VERSION, &chapter.to_bytes());
        Ok(chapter)
    }

    /// Translates through any service, using and filling the cache. The
    /// database lock is never held during the network call.
    fn translate_with(
        &self,
        translator: &dyn Translator,
        text: &str,
        context: Option<&str>,
        source_lang: &str,
        target_lang: &str,
    ) -> Result<Translation, AllreadsError> {
        let key = CacheKey::new(text, context, source_lang, target_lang, translator.name());
        if key.text.is_empty() {
            return Err(AllreadsError::InvalidInput {
                reason: "nothing to translate".into(),
            });
        }
        if let Some(cached) = self.db()?.cached_translation(&key)? {
            return Ok(Translation {
                text: cached,
                from_cache: true,
            });
        }
        let translated = translator.translate(&TranslationRequest {
            text: &key.text,
            context: key.context(),
            source: &key.source_lang,
            target: &key.target_lang,
        })?;
        self.db()?.save_translation(&key, &translated)?;
        Ok(Translation {
            text: translated,
            from_cache: false,
        })
    }

    fn book_row(&self, book_id: i64) -> Result<BookRow, AllreadsError> {
        self.db()?
            .book(book_id)?
            .ok_or_else(|| AllreadsError::Missing {
                what: format!("book {book_id}"),
            })
    }

    fn summary(db: &Database, row: BookRow) -> Result<BookSummary, AllreadsError> {
        let chapter_count = saturate_u32(db.chapter_titles(row.id)?.len());
        Ok(BookSummary {
            id: row.id,
            title: row.title,
            author: row.author,
            language_code: row.language_code,
            cover_path: row.cover_path,
            chapter_count,
            chapter_index: row.chapter_index,
            paragraph_index: row.paragraph_index,
            last_read_at: row.last_read_at,
        })
    }

    /// Deletes `path` only if it lies inside the app's data folder. Files the
    /// app did not copy in are never touched.
    fn remove_owned_file(&self, path: &Path) {
        let (Ok(file), Ok(data_dir)) = (path.canonicalize(), self.data_dir.canonicalize()) else {
            return;
        };
        if file.starts_with(&data_dir) {
            let _ = fs::remove_file(file);
        }
    }
}

// UniFFI exported methods must take owned arguments, so `String` stays by value.
#[uniffi::export]
#[allow(clippy::needless_pass_by_value)]
impl App {
    /// Opens the library in `data_dir`, creating the database on first run and
    /// adding any built-in language that is missing.
    ///
    /// # Errors
    /// Returns `DataFolderMissing` if the folder does not exist, and `Database`
    /// if the database cannot be opened or migrated.
    #[uniffi::constructor]
    pub fn new(data_dir: String) -> Result<Arc<Self>, AllreadsError> {
        let folder = PathBuf::from(&data_dir);
        if !folder.is_dir() {
            return Err(AllreadsError::DataFolderMissing { path: data_dir });
        }
        let db = Database::open(&folder.join("allreads.db"))?;
        let presets = builtin_presets().map_err(|e| AllreadsError::Database {
            reason: e.to_string(),
        })?;
        for preset in &presets {
            db.sync_builtin_language(preset)?;
        }
        Ok(Arc::new(Self {
            db: Mutex::new(db),
            data_dir: folder,
            open_books: Mutex::new(Vec::new()),
            dictionaries: Mutex::new(HashMap::new()),
        }))
    }

    /// The version of the database schema. Useful to check after an update.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn schema_version(&self) -> Result<i64, AllreadsError> {
        Ok(self.db()?.schema_version()?)
    }

    /// The codes of the languages books can be imported in, sorted.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn language_codes(&self) -> Result<Vec<String>, AllreadsError> {
        Ok(self
            .db()?
            .languages()?
            .into_iter()
            .map(|language| language.code)
            .collect())
    }

    /// Details of a stored language, or `None` if it is not stored.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn language_info(
        &self,
        language_code: String,
    ) -> Result<Option<LanguageInfo>, AllreadsError> {
        Ok(self
            .db()?
            .language(&language_code)?
            .map(|language| LanguageInfo {
                code: language.code,
                name: language.name,
                dictionary_url: language.dictionary_url,
                tts_locale: language.tts_locale,
                right_to_left: language.rtl,
            }))
    }

    /// Every saved word in a language, sorted by term key, for the Words screen.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn list_terms(&self, language_code: String) -> Result<Vec<SavedTerm>, AllreadsError> {
        Ok(self
            .db()?
            .terms(&language_code)?
            .into_iter()
            .map(SavedTerm::from)
            .collect())
    }

    /// Imports the book at `path`, which Kotlin has already copied into app
    /// storage, in the given language.
    ///
    /// # Errors
    /// Returns `NotFound`, `UnsupportedFile` or `InvalidBook` for the file,
    /// `Missing` for an unknown language, and `Storage` if the cover cannot be saved.
    pub fn import_book(
        &self,
        path: String,
        language_code: String,
    ) -> Result<BookSummary, AllreadsError> {
        // The slow part (parsing) happens before the database lock is taken.
        let prepared = library::prepare_import(Path::new(&path))?;
        let db = self.db()?;
        let id = library::store_import(&db, &self.covers_dir(), &prepared, &language_code)?;
        let row = db.book(id)?.ok_or_else(|| AllreadsError::Missing {
            what: format!("book {id}"),
        })?;
        let summary = Self::summary(&db, row)?;
        drop(db);
        self.remember_open_book(id, Arc::new(prepared.book))?;
        Ok(summary)
    }

    /// The word an import would take from a field, for the import screen's
    /// samples: "le chat (m.)" → "chat". `None` if nothing usable is left.
    ///
    /// # Errors
    /// Returns `Missing` for an unknown language.
    pub fn clean_import_word(
        &self,
        language_code: String,
        raw: String,
    ) -> Result<Option<String>, AllreadsError> {
        let articles = self.articles(&language_code)?;
        Ok(allreads_core::import::clean_word(&raw, &articles))
    }

    /// Reads a file to import words from: an Anki deck (`.apkg`, `.colpkg`),
    /// one group per note type with review progress; or a text export (Anki's
    /// "Notes in Plain Text", CSV, TSV), one group named after the file.
    /// `name` is the file's name as the user picked it.
    ///
    /// # Errors
    /// Returns `InvalidInput` for a file that cannot be read as either, and
    /// `Storage` if it cannot be opened.
    pub fn read_import_file(
        &self,
        path: String,
        name: String,
    ) -> Result<Vec<ImportGroup>, AllreadsError> {
        let lower = name.to_lowercase();
        if lower.ends_with(".apkg") || lower.ends_with(".colpkg") {
            let sources =
                allreads_core::apkg::read_apkg(Path::new(&path), &self.data_dir.join("cache"))?;
            return Ok(sources.into_iter().map(ImportGroup::from).collect());
        }
        let bytes = fs::read(&path).map_err(|e| AllreadsError::Storage {
            reason: e.to_string(),
        })?;
        // Text exports are UTF-8; anything else is read with replacement characters.
        let text = String::from_utf8_lossy(&bytes);
        let source = allreads_core::import::read_text_export(&text, &name);
        if source.notes.is_empty() {
            return Err(AllreadsError::InvalidInput {
                reason: "the file holds no rows to import".into(),
            });
        }
        Ok(vec![source.into()])
    }

    /// Imports words picked from notes: each row's word is cleaned, its status
    /// set by `status`, and saved under "the more known status wins". `source`
    /// is the badge ("Anki · French vocabulary"); with `from_anki`, the words
    /// already have cards and are never sent to AnkiDroid.
    ///
    /// # Errors
    /// Returns `Missing` for an unknown language, `InvalidInput` for a fixed
    /// status of New, and `Database` if saving fails (then nothing is saved).
    pub fn import_words(
        &self,
        language_code: String,
        source: String,
        from_anki: bool,
        rows: Vec<ImportRow>,
        status: ImportStatus,
    ) -> Result<ImportSummary, AllreadsError> {
        use allreads_core::db::import::ImportedWord;
        use allreads_core::import::{clean_meaning, clean_word, status_for};
        let fixed = match status {
            ImportStatus::Fixed { status } => {
                Some(
                    status
                        .to_status()?
                        .ok_or_else(|| AllreadsError::InvalidInput {
                            reason: "an import cannot mark words as new".into(),
                        })?,
                )
            }
            ImportStatus::FollowReviews => None,
        };
        let articles = self.articles(&language_code)?;
        let mut skipped = 0;
        let mut words = Vec::new();
        for row in rows {
            let progress = row
                .progress
                .map_or(allreads_core::import::CardProgress::New, Into::into);
            let (Some(text), Some(status)) = (
                clean_word(&row.word, &articles),
                fixed.or_else(|| status_for(progress)),
            ) else {
                skipped += 1;
                continue;
            };
            words.push(ImportedWord {
                text,
                meaning: row.meaning.as_deref().and_then(clean_meaning),
                status,
            });
        }
        let done = self
            .db()?
            .import_words(&language_code, source.trim(), from_anki, &words)?;
        Ok(ImportSummary {
            added: done.added,
            upgraded: done.upgraded,
            unchanged: done.unchanged,
            skipped,
        })
    }

    /// A free path in the books folder for a file the user picked, named after
    /// it (`name`, e.g. "Notes.txt", without its extension), so a TXT book's
    /// title is its file name. Kotlin copies the picked file there.
    pub fn new_book_path(&self, name: Option<String>, extension: String) -> String {
        let title = name
            .as_deref()
            .map_or("", |n| n.rsplit_once('.').map_or(n, |(stem, _)| stem));
        let dir = self.books_dir();
        let _ = fs::create_dir_all(&dir);
        library::book_file_path(&dir, title, &extension)
            .display()
            .to_string()
    }

    /// Writes pasted or shared text as a TXT book, ready to import. Without a
    /// title, its first words become one.
    ///
    /// # Errors
    /// Returns `InvalidInput` for empty text, and `Storage` if it cannot be written.
    pub fn prepare_text(
        &self,
        title: Option<String>,
        text: String,
    ) -> Result<PreparedText, AllreadsError> {
        let paragraphs = allreads_core::txt::split_paragraphs(&text);
        if paragraphs.is_empty() {
            return Err(AllreadsError::InvalidInput {
                reason: "there is no text to import".into(),
            });
        }
        let title = title
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
            .unwrap_or_else(|| library::title_from_text(&text));
        self.write_text(title, None, &paragraphs)
    }

    /// Picks the article out of a web page and writes it as a TXT book, titled
    /// with its headline, ready to import. `page` is the page as fetched; its
    /// character set comes from `content_type` (the HTTP header) or the page.
    ///
    /// # Errors
    /// Returns `InvalidInput` if the page has no readable text, and `Storage`
    /// if it cannot be written.
    pub fn prepare_article(
        &self,
        page: Vec<u8>,
        content_type: Option<String>,
    ) -> Result<PreparedText, AllreadsError> {
        let html = allreads_core::article::decode_html(&page, content_type.as_deref());
        let article = allreads_core::article::extract(&html);
        if article.paragraphs.is_empty() {
            return Err(AllreadsError::InvalidInput {
                reason: "no readable text was found on that page".into(),
            });
        }
        self.write_text(article.title, article.language, &article.paragraphs)
    }

    /// How hard a book is: its words against the saved ones. The first call
    /// for a book reads every chapter (slow for a long book); the counts are
    /// stored, so later calls are one query.
    ///
    /// # Errors
    /// Returns `Missing` for an unknown book, and a book or database error if
    /// a chapter cannot be read.
    pub fn book_difficulty(&self, book_id: i64) -> Result<BookDifficulty, AllreadsError> {
        self.ensure_book_counted(book_id)?;
        Ok(self.db()?.book_difficulty(book_id)?.into())
    }

    /// Counts a book's words if they are not counted yet, or were counted with
    /// other rules or another dictionary. Returns `true` if it counted now (the
    /// reader then reloads its marks for names). The first count of a long
    /// book reads every chapter.
    ///
    /// # Errors
    /// Returns `Missing` for an unknown book, and a book or dictionary error if
    /// a chapter or a lookup fails (nothing is stored then).
    pub fn ensure_book_counted(&self, book_id: i64) -> Result<bool, AllreadsError> {
        let row = self.book_row(book_id)?;
        let stamp = self.count_stamp(&row.language_code)?;
        if self.db()?.book_count_stamp(book_id)?.as_deref() == Some(stamp.as_str()) {
            return Ok(false);
        }
        let rules = self.rules(&row.language_code)?;
        let book = self.opened_book(book_id)?;
        let chapters = self.db()?.chapter_titles(book_id)?.len();
        let has_dictionary = !stamp.ends_with('/');
        // One lookup per word; the dictionary is locked only for each lookup.
        let lookup = |key: &str| {
            self.with_dictionary(&row.language_code, |d| d.word_kinds(key))
                .map(Option::unwrap_or_default)
                .map_err(|e| DictionaryError::Invalid(e.to_string()))
        };
        let words = word_tally::count_book(
            &book,
            chapters,
            &rules,
            &row.language_code,
            has_dictionary.then_some(&lookup as &dyn Fn(&str) -> _),
        )?;
        // Stored with the stamp from before counting: if the dictionary
        // changed meanwhile, the next call sees a new stamp and counts again.
        self.db()?.store_book_words(book_id, &stamp, &words)?;
        Ok(true)
    }

    /// The languages for the Words tab: those of the books and of the saved
    /// words, the most recently read first.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn word_language_codes(&self) -> Result<Vec<String>, AllreadsError> {
        Ok(self.db()?.word_languages()?)
    }

    /// The difficulty of every book counted with today's rules, in one query,
    /// so the library can show them at once; books missing from it still need
    /// [`App::book_difficulty`].
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn counted_difficulties(&self) -> Result<Vec<BookDifficultyOf>, AllreadsError> {
        let counted = self.db()?.counted_difficulties()?;
        // Today's stamp per language, worked out once per language.
        let mut stamps: HashMap<String, String> = HashMap::new();
        let mut current = Vec::new();
        for book in counted {
            if !stamps.contains_key(&book.language_code) {
                let stamp = self.count_stamp(&book.language_code)?;
                stamps.insert(book.language_code.clone(), stamp);
            }
            if stamps[&book.language_code] == book.stamp {
                current.push(BookDifficultyOf {
                    book_id: book.book_id,
                    difficulty: book.difficulty.into(),
                });
            }
        }
        Ok(current)
    }

    /// Changes only a saved word's status, e.g. a double tap raising its level;
    /// its translation, notes and card stay. Returns the old status for undo,
    /// or `None` if the word is not saved.
    ///
    /// # Errors
    /// Returns `Missing` for an unknown language, and `InvalidInput` for a
    /// learning level outside 1 to 5.
    pub fn set_term_status(
        &self,
        language_code: String,
        text: String,
        status: WordStatus,
    ) -> Result<Option<WordStatus>, AllreadsError> {
        let status = status
            .to_status()?
            .ok_or_else(|| AllreadsError::InvalidInput {
                reason: "use forget_term to un-save a word".into(),
            })?;
        Ok(self
            .db()?
            .set_term_status(&language_code, &text, status)?
            .map(WordStatus::from))
    }

    /// A language's saved-word counts for each of the last 90 days, with the
    /// known words gained this week, for the progress chart.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn word_progress(&self, language_code: String) -> Result<WordProgress, AllreadsError> {
        Ok(self.db()?.word_progress(&language_code, 90)?.into())
    }

    /// Every book in the library: recently read first, then unread books, newest first.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn library(&self) -> Result<Vec<BookSummary>, AllreadsError> {
        let db = self.db()?;
        db.books()?
            .into_iter()
            .map(|row| Self::summary(&db, row))
            .collect()
    }

    /// One book, or `None` if it is not in the library.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn book(&self, book_id: i64) -> Result<Option<BookSummary>, AllreadsError> {
        let db = self.db()?;
        db.book(book_id)?
            .map(|row| Self::summary(&db, row))
            .transpose()
    }

    /// The book's chapter titles in reading order. `None` means the chapter has
    /// no heading; show "Chapter N" instead.
    ///
    /// # Errors
    /// Returns `Missing` if the book does not exist.
    pub fn chapter_titles(&self, book_id: i64) -> Result<Vec<Option<String>>, AllreadsError> {
        let db = self.db()?;
        if db.book(book_id)?.is_none() {
            return Err(AllreadsError::Missing {
                what: format!("book {book_id}"),
            });
        }
        Ok(db.chapter_titles(book_id)?)
    }

    /// One chapter as blocks of tokens, each word with its term key and status.
    /// The parse is cached; statuses are always read fresh, so call this again
    /// after saving a word to recolour the chapter.
    ///
    /// # Errors
    /// Returns `Missing` for an unknown book or language, `NotFound` if the book
    /// file is gone, and `ChapterOutOfRange` for a bad index.
    pub fn open_chapter(&self, book_id: i64, index: u32) -> Result<Chapter, AllreadsError> {
        let row = self.book_row(book_id)?;
        let parsed = self.parsed_chapter(&row, index)?;
        let statuses = self.db()?.term_statuses(&row.language_code)?;
        // From the book's counted words (empty until its difficulty was asked for).
        // Only from a count that is still current: after a dictionary change,
        // names and forms wait for the new count (the reader asks for one).
        let stamp = self.count_stamp(&row.language_code)?;
        let (names, lemmas) = {
            let db = self.db()?;
            if db.book_count_stamp(book_id)?.as_deref() == Some(stamp.as_str()) {
                (db.book_names(book_id)?, db.book_lemmas(book_id)?)
            } else {
                Default::default()
            }
        };
        let status_of = |key: &str| {
            statuses
                .get(key)
                .map_or(WordStatus::New, |status| (*status).into())
        };
        let mut next_phrase_id = 0;
        let blocks = parsed
            .blocks
            .into_iter()
            .map(|block| {
                // Saved phrases in this block, by token range.
                let slots: Vec<WordSlot> = block
                    .tokens
                    .iter()
                    .map(|t| WordSlot {
                        key: t.key.as_deref(),
                        sentence: t.sentence,
                    })
                    .collect();
                let spans = find_phrases(&slots, &|key| statuses.contains_key(key));
                let mut marks: Vec<Option<PhraseMark>> = vec![None; block.tokens.len()];
                for span in spans {
                    let mark = PhraseMark {
                        id: next_phrase_id,
                        status: status_of(&span.key),
                        term_key: span.key,
                    };
                    next_phrase_id += 1;
                    for slot in &mut marks[span.first..=span.last] {
                        *slot = Some(mark.clone());
                    }
                }
                ChapterBlock {
                    heading_level: block.heading_level,
                    tokens: block
                        .tokens
                        .into_iter()
                        .zip(marks)
                        .map(|(token, phrase)| Token {
                            status: token.key.as_deref().map(status_of),
                            name: token.key.as_ref().is_some_and(|k| names.contains(k)),
                            form_of_saved: token
                                .key
                                .as_ref()
                                .and_then(|k| lemmas.get(k))
                                .is_some_and(|lemma| statuses.contains_key(lemma)),
                            text: token.text,
                            term_key: token.key,
                            sentence: token.sentence,
                            italic: token.italic,
                            bold: token.bold,
                            phrase,
                        })
                        .collect(),
                }
            })
            .collect();
        Ok(Chapter {
            book_id,
            index,
            blocks,
        })
    }

    /// Phrases the offline dictionary knows around a tapped word: `words` are
    /// the sentence's words in order, `index` the tapped one. Empty when no
    /// dictionary is installed for the language.
    ///
    /// # Errors
    /// Returns `Missing` for an unknown language, and `DictionaryInvalid` if
    /// the installed file cannot be read.
    pub fn phrase_suggestions(
        &self,
        language_code: String,
        words: Vec<String>,
        index: u32,
    ) -> Result<Vec<PhraseSuggestion>, AllreadsError> {
        let language =
            self.db()?
                .language(&language_code)?
                .ok_or_else(|| AllreadsError::Missing {
                    what: format!("language {language_code}"),
                })?;
        let rules = LanguageRules::new(&language)
            .map_err(|reason| AllreadsError::InvalidInput { reason })?;
        let keys: Vec<String> = words.iter().map(|w| rules.key(w)).collect();
        let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
        let index = usize::try_from(index).unwrap_or(usize::MAX);
        let hits = self.with_dictionary(&language_code, |dictionary| {
            dictionary.phrases_around(&keys, index)
        })?;
        Ok(hits
            .unwrap_or_default()
            .into_iter()
            .map(|hit| PhraseSuggestion {
                first_word: saturate_u32(hit.first),
                last_word: saturate_u32(hit.last),
                text: hit.word,
                term_key: hit.key,
                gloss: hit.gloss,
            })
            .collect())
    }

    /// Saves a word (or phrase) with its status and translation. Saving a word
    /// that is already saved updates it. Call `open_chapter` again afterwards to
    /// recolour every occurrence.
    ///
    /// # Errors
    /// Returns `InvalidInput` for an empty word, the `New` status or a bad level,
    /// and `Missing` for an unknown language.
    pub fn save_term(&self, input: TermInput) -> Result<SavedTerm, AllreadsError> {
        let status = input
            .status
            .to_status()?
            .ok_or_else(|| AllreadsError::InvalidInput {
                reason: "a saved word cannot be New; use forget_term".into(),
            })?;
        // "Coq" at a sentence start is saved as "coq" when the dictionary knows
        // the common word; names and words it does not know stay as written.
        let text = {
            let key = self
                .db()?
                .term_key(&input.language_code, input.text.trim())?;
            let kinds = match key {
                Some(key) => self.with_dictionary(&input.language_code, |d| d.word_kinds(&key))?,
                None => None,
            };
            common_form(input.text.trim(), kinds.as_ref(), &input.language_code)
        };
        let db = self.db()?;
        let context = input.context();
        let id = db.save_term(&NewTerm {
            context,
            language_code: input.language_code,
            text,
            lemma: input.lemma,
            status,
            translation: input.translation,
            notes: input.notes,
            parent_id: None,
        })?;
        let term = db.term(id)?.ok_or_else(|| AllreadsError::Missing {
            what: format!("term {id}"),
        })?;
        Ok(term.into())
    }

    /// The saved term for a word, for the word sheet, or `None` if it is new.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn find_term(
        &self,
        language_code: String,
        text: String,
    ) -> Result<Option<SavedTerm>, AllreadsError> {
        Ok(self
            .db()?
            .find_term(&language_code, &text)?
            .map(SavedTerm::from))
    }

    /// Un-saves a word, so it shows as new again. Returns `false` if it was not
    /// saved. Its card stays in `AnkiDroid`.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be written.
    pub fn forget_term(&self, language_code: String, text: String) -> Result<bool, AllreadsError> {
        let db = self.db()?;
        let Some(term) = db.find_term(&language_code, &text)? else {
            return Ok(false);
        };
        db.delete_term(term.id)?;
        Ok(true)
    }

    /// Marks every new word in a chapter, from block `from_block` to the end,
    /// as well known (PRD F8). Saved words keep their status. Returns how many
    /// words were added and their term ids, for [`App::undo_mark_known`].
    ///
    /// # Errors
    /// Returns `Missing` for an unknown book, and `ChapterOutOfRange` for a bad index.
    pub fn mark_rest_known(
        &self,
        book_id: i64,
        chapter_index: u32,
        from_block: u32,
    ) -> Result<MarkedKnown, AllreadsError> {
        let row = self.book_row(book_id)?;
        let parsed = self.parsed_chapter(&row, chapter_index)?;
        let words: Vec<(String, String)> = parsed
            .blocks
            .iter()
            .skip(from_block as usize)
            .flat_map(|block| &block.tokens)
            .filter_map(|token| Some((token.key.clone()?, token.text.clone())))
            .collect();
        let term_ids = self.db()?.mark_known(&row.language_code, &words)?;
        Ok(MarkedKnown { term_ids })
    }

    /// Undoes [`App::mark_rest_known`], keeping any word whose status was
    /// changed since. Returns how many words went back to new.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be written.
    pub fn undo_mark_known(&self, term_ids: Vec<i64>) -> Result<u32, AllreadsError> {
        Ok(saturate_u32(self.db()?.undo_mark_known(&term_ids)?))
    }

    /// Translates a word or sentence with DeepL. Repeats come from the cache.
    /// For a tapped word, pass its sentence as `context`: DeepL uses it to pick
    /// the right sense ("lit" as "reads" or "bed") and does not bill it.
    /// `deepl_api_key` comes from the Android Keystore for each call; it is
    /// never stored or logged.
    ///
    /// # Errors
    /// Returns `InvalidInput` for empty text or key, `TranslationUnavailable`
    /// when offline, `TranslationKeyRejected`, `TranslationQuotaExceeded`,
    /// `TranslationRateLimited`, or `TranslationFailed`.
    pub fn translate(
        &self,
        text: String,
        context: Option<String>,
        source_lang: String,
        target_lang: String,
        deepl_api_key: String,
    ) -> Result<Translation, AllreadsError> {
        if deepl_api_key.trim().is_empty() {
            return Err(AllreadsError::InvalidInput {
                reason: "a DeepL API key is needed".into(),
            });
        }
        let deepl = DeepL::new(&deepl_api_key);
        self.translate_with(
            &deepl,
            &text,
            context.as_deref(),
            &source_lang,
            &target_lang,
        )
    }

    /// Saves where the reader stopped in a book.
    ///
    /// # Errors
    /// Returns `Missing` if the book does not exist.
    pub fn save_position(
        &self,
        book_id: i64,
        chapter_index: u32,
        paragraph_index: u32,
    ) -> Result<(), AllreadsError> {
        Ok(self
            .db()?
            .save_position(book_id, chapter_index, paragraph_index)?)
    }

    /// The Anki note type for a language. Kotlin creates it in `AnkiDroid`
    /// once and stores its id with `set_anki_model_id`.
    ///
    /// # Errors
    /// Returns `Missing` for an unknown language.
    pub fn anki_note_type(
        &self,
        language_code: String,
        include_word_card: bool,
    ) -> Result<AnkiNoteType, AllreadsError> {
        let language =
            self.db()?
                .language(&language_code)?
                .ok_or_else(|| AllreadsError::Missing {
                    what: format!("language {language_code}"),
                })?;
        Ok(allreads_core::anki::note_type(&language, include_word_card).into())
    }

    /// Every note waiting to go to `AnkiDroid`, oldest first.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn pending_anki_notes(&self) -> Result<Vec<AnkiNote>, AllreadsError> {
        Ok(allreads_core::anki::pending_notes(&*self.db()?)?
            .into_iter()
            .map(AnkiNote::from)
            .collect())
    }

    /// Records that a note reached `AnkiDroid`, so it leaves the queue and
    /// later edits update the same note. Pass the note's `queue_id`: a word
    /// edited during the push stays queued.
    ///
    /// # Errors
    /// Returns `Missing` if the term no longer exists.
    pub fn record_anki_push(
        &self,
        term_id: i64,
        queue_id: i64,
        note_id: i64,
        deck_id: i64,
    ) -> Result<(), AllreadsError> {
        Ok(self
            .db()?
            .record_anki_push(term_id, queue_id, note_id, deck_id)?)
    }

    /// How many words are waiting to go to `AnkiDroid`, for Settings.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn pending_anki_count(&self) -> Result<u32, AllreadsError> {
        Ok(saturate_u32(self.db()?.anki_queue()?.len()))
    }

    /// Records a failed push; the note stays queued for the next try.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be written.
    pub fn record_anki_failure(&self, term_id: i64) -> Result<(), AllreadsError> {
        Ok(self.db()?.record_anki_attempt(term_id)?)
    }

    /// The `AnkiDroid` deck chosen in Settings, if any.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn anki_deck_id(&self) -> Result<Option<i64>, AllreadsError> {
        self.setting_id(ANKI_DECK)
    }

    /// Stores the `AnkiDroid` deck chosen in Settings.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be written.
    pub fn set_anki_deck_id(&self, deck_id: i64) -> Result<(), AllreadsError> {
        Ok(self.db()?.set_setting(ANKI_DECK, &deck_id.to_string())?)
    }

    /// The `AnkiDroid` note type id made for a language, if any.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be read.
    pub fn anki_model_id(&self, language_code: String) -> Result<Option<i64>, AllreadsError> {
        self.setting_id(&model_key(&language_code))
    }

    /// Stores the `AnkiDroid` note type id made for a language.
    ///
    /// # Errors
    /// Returns `Database` if the database cannot be written.
    pub fn set_anki_model_id(
        &self,
        language_code: String,
        model_id: i64,
    ) -> Result<(), AllreadsError> {
        Ok(self
            .db()?
            .set_setting(&model_key(&language_code), &model_id.to_string())?)
    }

    /// The installed offline dictionaries, for Settings.
    ///
    /// # Errors
    /// Returns `Storage` if the dictionaries folder cannot be read.
    pub fn installed_dictionaries(&self) -> Result<Vec<InstalledDictionary>, AllreadsError> {
        Ok(dictionary::installed(&self.dictionaries_dir())?
            .into_iter()
            .map(InstalledDictionary::from)
            .collect())
    }

    /// Installs a downloaded dictionary (the `.gz` at `download_path`) after
    /// checking it against the manifest's `sha256`. Kotlin deletes the
    /// download afterwards, whatever the result.
    ///
    /// # Errors
    /// Returns `DictionaryDamaged` for a corrupt download, `DictionaryInvalid`
    /// for a file that is not a usable dictionary for `pair`, and `Storage` if
    /// it cannot be written.
    pub fn install_dictionary(
        &self,
        download_path: String,
        sha256: String,
        pair: String,
    ) -> Result<InstalledDictionary, AllreadsError> {
        // Close an open copy first, so the new file is opened on next lookup.
        self.dictionaries
            .lock()
            .map_err(|_| poisoned())?
            .remove(&pair);
        let info = dictionary::install(
            Path::new(&download_path),
            &sha256,
            &pair,
            &self.dictionaries_dir(),
        )?;
        Ok(info.into())
    }

    /// Deletes an installed dictionary. Returns `false` if it was not installed.
    ///
    /// # Errors
    /// Returns `DictionaryInvalid` for a bad pair name, `Storage` if deleting fails.
    pub fn remove_dictionary(&self, pair: String) -> Result<bool, AllreadsError> {
        self.dictionaries
            .lock()
            .map_err(|_| poisoned())?
            .remove(&pair);
        Ok(dictionary::remove(&self.dictionaries_dir(), &pair)?)
    }

    /// Looks a word up in the language's `-en` dictionary. `None` if no
    /// dictionary is installed for it; an empty lookup if the word is not in it.
    ///
    /// # Errors
    /// Returns `Missing` for an unknown language, and `DictionaryInvalid` if
    /// the installed file cannot be read.
    pub fn lookup_word(
        &self,
        language_code: String,
        text: String,
    ) -> Result<Option<DictionaryLookup>, AllreadsError> {
        let key = self
            .db()?
            .term_key(&language_code, text.trim())?
            .ok_or_else(|| AllreadsError::Missing {
                what: format!("language {language_code}"),
            })?;
        let lookup = self.with_dictionary(&language_code, |dictionary| dictionary.lookup(&key))?;
        Ok(lookup.map(|lookup| {
            let common = common_form(text.trim(), Some(&lookup.kinds()), &language_code);
            DictionaryLookup::new(lookup, common)
        }))
    }

    /// Deletes a book, its cover, and its file if the file is in app storage.
    ///
    /// # Errors
    /// Returns `Missing` if the book does not exist.
    pub fn delete_book(&self, book_id: i64) -> Result<(), AllreadsError> {
        let row = self.book_row(book_id)?;
        self.db()?.delete_book(book_id)?;
        self.open_books
            .lock()
            .map_err(|_| poisoned())?
            .retain(|(id, _)| *id != book_id);
        if let Some(cover) = &row.cover_path {
            self.remove_owned_file(Path::new(cover));
        }
        self.remove_owned_file(Path::new(&row.file_path));
        Ok(())
    }
}

const ANKI_DECK: &str = "anki.deck";

fn model_key(language_code: &str) -> String {
    format!("anki.model.{}", language_code.to_lowercase())
}

/// Reads the published `dictionaries.json`, keeping the dictionaries this
/// app version can use.
///
/// # Errors
/// Returns `DictionaryInvalid` if the text is not a manifest.
#[uniffi::export]
#[allow(clippy::needless_pass_by_value)]
pub fn parse_dictionary_manifest(json: String) -> Result<Vec<AvailableDictionary>, AllreadsError> {
    Ok(dictionary::parse_manifest(&json)?
        .into_iter()
        .map(AvailableDictionary::from)
        .collect())
}

/// Checks a DeepL key right after the user enters it. It first reads the
/// key's usage, which costs no quota. A scoped key may be allowed to translate
/// but not to read usage, so if DeepL refuses that, it translates "ok" (two
/// characters of quota) and the key counts as working if that succeeds.
///
/// # Errors
/// Returns `TranslationKeyRejected` if DeepL refuses both, `TranslationUnavailable`
/// when offline, and `InvalidInput` for an empty key.
// UniFFI exported functions must take owned arguments, so `String` stays by value.
#[uniffi::export]
#[allow(clippy::needless_pass_by_value)]
pub fn check_deepl_key(api_key: String) -> Result<DeepLKeyStatus, AllreadsError> {
    if api_key.trim().is_empty() {
        return Err(AllreadsError::InvalidInput {
            reason: "the DeepL API key is empty".into(),
        });
    }
    check_with(&DeepL::new(&api_key))
}

/// The key check against any DeepL endpoint, so tests can use a local server.
fn check_with(deepl: &DeepL) -> Result<DeepLKeyStatus, AllreadsError> {
    match deepl.usage() {
        Ok(usage) => Ok(DeepLKeyStatus {
            characters_used: Some(usage.character_count),
            character_limit: Some(usage.character_limit),
        }),
        Err(TranslateError::Unauthorized) => {
            deepl.translate(&TranslationRequest {
                text: "ok",
                context: None,
                source: "en",
                target: "de",
            })?;
            Ok(DeepLKeyStatus {
                characters_used: None,
                character_limit: None,
            })
        }
        Err(other) => Err(other.into()),
    }
}

/// Reads a book file's title, author, language and chapter count without
/// importing it, e.g. to suggest the language on the import screen.
///
/// # Errors
/// Returns `NotFound`, `UnsupportedFile` or `InvalidBook`.
// UniFFI exported functions must take owned arguments, so `String` stays by value.
#[uniffi::export]
#[allow(clippy::needless_pass_by_value)]
pub fn read_book_meta(path: String) -> Result<BookMeta, AllreadsError> {
    Ok(OpenedBook::open(Path::new(&path))?.meta().into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::records::CardProgress;
    use allreads_core::db::term::Status;
    use allreads_core::db::SCHEMA_VERSION;

    fn fixture(name: &str) -> String {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../core/tests/fixtures")
            .join(name)
            .display()
            .to_string()
    }

    fn new_app(dir: &tempfile::TempDir) -> Arc<App> {
        App::new(dir.path().display().to_string()).expect("app opens")
    }

    /// Copies a fixture into the app's data folder, as Kotlin's file picker would.
    fn copy_in(dir: &tempfile::TempDir, name: &str) -> String {
        let books = dir.path().join("books");
        fs::create_dir_all(&books).unwrap();
        let target = books.join(name);
        fs::copy(fixture(name), &target).unwrap();
        target.display().to_string()
    }

    #[test]
    fn app_creates_the_database_and_built_in_languages_on_first_run() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        assert_eq!(app.schema_version().unwrap(), SCHEMA_VERSION);
        assert!(dir.path().join("allreads.db").is_file());
        assert_eq!(app.language_codes().unwrap(), ["de", "en", "es", "fr"]);
    }

    #[test]
    fn app_refuses_a_missing_data_folder() {
        let result = App::new("/no/such/allreads/folder".to_string());
        assert!(matches!(
            result,
            Err(AllreadsError::DataFolderMissing { .. })
        ));
    }

    #[test]
    fn imported_books_survive_restarting_the_app() {
        let dir = tempfile::tempdir().unwrap();
        {
            let app = new_app(&dir);
            let summary = app
                .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
                .unwrap();
            assert_eq!(summary.title, "Le Petit Test");
            assert_eq!(summary.chapter_count, 2);
            app.import_book(copy_in(&dir, "sample.txt"), "fr".into())
                .unwrap();
            app.save_position(summary.id, 1, 3).unwrap();
        }
        let app = new_app(&dir);
        let library = app.library().unwrap();
        let titles: Vec<&str> = library.iter().map(|b| b.title.as_str()).collect();
        assert_eq!(titles, ["Le Petit Test", "sample"]);
        assert_eq!(
            (library[0].chapter_index, library[0].paragraph_index),
            (1, 3)
        );
        assert_eq!(library[1].chapter_count, 1);
    }

    #[test]
    fn chapters_are_read_by_book_id_after_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let id = new_app(&dir)
            .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
            .unwrap()
            .id;
        let app = new_app(&dir);
        let chapter = app.open_chapter(id, 1).unwrap();
        let texts: Vec<String> = chapter
            .blocks
            .iter()
            .map(|b| b.tokens.iter().map(|t| t.text.as_str()).collect())
            .collect();
        assert_eq!(texts, ["Chapitre deux", "La suite commence ici."]);
        assert_eq!(
            app.chapter_titles(id).unwrap(),
            [
                Some("Chapitre un".to_string()),
                Some("Chapitre deux".to_string())
            ]
        );
        assert!(matches!(
            app.open_chapter(id, 9),
            Err(AllreadsError::ChapterOutOfRange { index: 9, count: 2 })
        ));
    }

    fn save_word(app: &App, text: &str, status: Status) {
        app.db()
            .unwrap()
            .save_term(&allreads_core::db::term::NewTerm {
                language_code: "fr".into(),
                text: text.into(),
                lemma: None,
                status,
                translation: String::new(),
                notes: String::new(),
                parent_id: None,
                context: None,
            })
            .unwrap();
    }

    fn word_statuses(chapter: &Chapter) -> Vec<(String, WordStatus)> {
        chapter
            .blocks
            .iter()
            .flat_map(|block| &block.tokens)
            .filter_map(|t| Some((t.text.clone(), t.status?)))
            .collect()
    }

    #[test]
    fn open_chapter_returns_blocks_of_tokens_with_statuses() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let id = app
            .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
            .unwrap()
            .id;
        save_word(&app, "Homme", Status::Learning(2));
        let chapter = app.open_chapter(id, 0).unwrap();
        assert_eq!(chapter.blocks[0].heading_level, Some(1));
        assert_eq!(chapter.blocks[1].heading_level, None);
        let statuses = word_statuses(&chapter);
        assert!(statuses.contains(&("homme".into(), WordStatus::Learning { level: 2 })));
        assert!(statuses.contains(&("lisait".into(), WordStatus::New)));
        let italic: Vec<&str> = chapter.blocks[2]
            .tokens
            .iter()
            .filter(|t| t.italic)
            .map(|t| t.text.as_str())
            .collect();
        assert_eq!(italic, ["livre"]);
    }

    #[test]
    fn non_words_have_no_key_or_status() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let id = app
            .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
            .unwrap()
            .id;
        let chapter = app.open_chapter(id, 0).unwrap();
        for token in chapter.blocks.iter().flat_map(|b| &b.tokens) {
            assert_eq!(
                token.term_key.is_some(),
                token.status.is_some(),
                "{token:?}"
            );
            if token.term_key.is_none() {
                assert!(!token.text.chars().any(char::is_alphabetic), "{token:?}");
            }
        }
    }

    #[test]
    fn reopening_uses_the_cache_but_reads_statuses_fresh() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let id = app
            .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
            .unwrap()
            .id;
        let first = app.open_chapter(id, 0).unwrap();
        assert!(app
            .db()
            .unwrap()
            .cached_chapter(id, 0, PARSER_VERSION)
            .unwrap()
            .is_some());
        save_word(&app, "livre", Status::WellKnown);
        let second = app.open_chapter(id, 0).unwrap();
        assert!(word_statuses(&first).contains(&("livre".into(), WordStatus::New)));
        assert!(word_statuses(&second).contains(&("livre".into(), WordStatus::WellKnown)));
    }

    #[test]
    fn a_corrupt_cache_entry_is_parsed_again() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let id = app
            .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
            .unwrap()
            .id;
        let good = app.open_chapter(id, 0).unwrap();
        app.db()
            .unwrap()
            .store_chapter_cache(id, 0, PARSER_VERSION, b"garbage")
            .unwrap();
        assert_eq!(app.open_chapter(id, 0).unwrap(), good);
    }

    #[test]
    fn text_books_open_as_one_chapter_of_paragraphs() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let id = app
            .import_book(copy_in(&dir, "sample.txt"), "fr".into())
            .unwrap()
            .id;
        let chapter = app.open_chapter(id, 0).unwrap();
        assert_eq!(chapter.blocks.len(), 2);
        assert!(chapter.blocks.iter().all(|b| b.heading_level.is_none()));
        assert!(matches!(
            app.open_chapter(id, 1),
            Err(AllreadsError::ChapterOutOfRange { index: 1, count: 1 })
        ));
    }

    fn input(text: &str, status: WordStatus) -> TermInput {
        TermInput {
            language_code: "fr".into(),
            text: text.into(),
            status,
            translation: "man".into(),
            notes: String::new(),
            lemma: None,
            sentence: None,
            sentence_translation: None,
            source_book: None,
            source_chapter: None,
        }
    }

    #[test]
    fn saved_words_wait_for_anki_with_their_sentence() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let saved = app
            .save_term(TermInput {
                sentence: Some("L’homme s’arrêta.".into()),
                sentence_translation: Some("The man stopped.".into()),
                source_book: Some("Le Livre".into()),
                source_chapter: Some("Un".into()),
                ..input("homme", WordStatus::Learning { level: 1 })
            })
            .unwrap();
        app.save_term(input("le", WordStatus::WellKnown)).unwrap();
        let notes = app.pending_anki_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(app.pending_anki_count().unwrap(), 1);
        assert_eq!(notes[0].term_id, saved.id);
        assert_eq!(notes[0].fields[3], "L’<b>homme</b> s’arrêta.");
        assert_eq!(notes[0].fields[5], "Le Livre — Un");

        app.record_anki_push(saved.id, notes[0].queue_id, 55, 1)
            .unwrap();
        assert!(app.pending_anki_notes().unwrap().is_empty());
        // Re-saving from the Words screen keeps the sentence and updates note 55.
        app.save_term(input("homme", WordStatus::Learning { level: 2 }))
            .unwrap();
        let notes = app.pending_anki_notes().unwrap();
        assert_eq!(notes[0].note_id, Some(55));
        assert_eq!(notes[0].fields[4], "The man stopped.");
        app.record_anki_failure(saved.id).unwrap();
    }

    /// A gzipped tiny French dictionary and its checksum, as the release would have.
    fn tiny_dictionary_download(dir: &std::path::Path) -> (String, String) {
        use std::io::Write;
        let plain = dir.join("plain.sqlite");
        let conn = rusqlite::Connection::open(&plain).unwrap();
        conn.execute_batch(allreads_core::dictionary::SCHEMA)
            .unwrap();
        conn.execute_batch(
            "INSERT INTO entry (id, key, word, pos, ipa) VALUES (1, 'lire', 'lire', 'verb', '/liʁ/'),
                 (2, 'pierre précieuse', 'pierre précieuse', 'noun', '');
             INSERT INTO sense (entry_id, position, gloss) VALUES (1, 0, 'to read'), (2, 0, 'gemstone');
             INSERT INTO form (key, lemma, pos, tags)
                 VALUES ('lisait', 'lire', 'verb', 'imperfect indicative singular third-person');
             INSERT INTO meta (key, value) VALUES ('format', '1'), ('pair', 'fr-en'), ('built', '2026-11-25');",
        )
        .unwrap();
        drop(conn);
        let gz = dir.join("fr-en.sqlite.gz");
        let mut encoder = flate2::write::GzEncoder::new(
            fs::File::create(&gz).unwrap(),
            flate2::Compression::fast(),
        );
        encoder.write_all(&fs::read(&plain).unwrap()).unwrap();
        encoder.finish().unwrap();
        let sum = allreads_core::dictionary::file_sha256(&gz).unwrap();
        (gz.display().to_string(), sum)
    }

    #[test]
    fn a_dictionary_installs_looks_words_up_and_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        // Nothing installed: no lookup, not an error.
        assert_eq!(app.lookup_word("fr".into(), "Lisait".into()).unwrap(), None);
        assert!(app.installed_dictionaries().unwrap().is_empty());

        let (gz, sum) = tiny_dictionary_download(dir.path());
        assert!(matches!(
            app.install_dictionary(gz.clone(), "00".into(), "fr-en".into()),
            Err(AllreadsError::DictionaryDamaged)
        ));
        let installed = app.install_dictionary(gz, sum, "fr-en".into()).unwrap();
        assert_eq!(installed.built, "2026-11-25");
        assert_eq!(app.installed_dictionaries().unwrap(), [installed]);

        // Looked up by term key, so case does not matter.
        let lookup = app
            .lookup_word("fr".into(), "Lisait".into())
            .unwrap()
            .unwrap();
        assert!(lookup.entries.is_empty());
        assert_eq!(lookup.suggested_lemma.as_deref(), Some("lire"));
        assert_eq!(
            lookup.forms[0].tags,
            "imperfect indicative singular third-person"
        );
        let lire = app
            .lookup_word("fr".into(), "lire".into())
            .unwrap()
            .unwrap();
        assert_eq!(lire.entries[0].senses, ["to read"]);
        assert_eq!(lire.entries[0].part_of_speech, "verb");
        // A word the dictionary does not know is an empty lookup.
        assert!(app
            .lookup_word("fr".into(), "xyz".into())
            .unwrap()
            .unwrap()
            .entries
            .is_empty());
        // Other languages have no dictionary.
        assert_eq!(app.lookup_word("de".into(), "Haus".into()).unwrap(), None);

        assert!(app.remove_dictionary("fr-en".into()).unwrap());
        assert_eq!(app.lookup_word("fr".into(), "lire".into()).unwrap(), None);
        assert!(matches!(
            app.remove_dictionary("../x".into()),
            Err(AllreadsError::DictionaryInvalid { .. })
        ));
    }

    #[test]
    fn the_manifest_is_parsed_for_kotlin() {
        let list = parse_dictionary_manifest(
            r#"{"format": 1, "dictionaries": [{"pair": "fr-en", "language": "fr",
               "url": "https://example.org/fr-en.sqlite.gz", "built": "2026-11-25",
               "download_bytes": 10, "installed_bytes": 40, "sha256": "ab"}]}"#
                .into(),
        )
        .unwrap();
        assert_eq!(list[0].language_code, "fr");
        assert!(parse_dictionary_manifest("nope".into()).is_err());
    }

    #[test]
    fn anki_note_types_and_ids_are_per_language() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let note_type = app.anki_note_type("fr".into(), true).unwrap();
        assert_eq!(note_type.name, "Allreads (French)");
        assert_eq!(note_type.cards.len(), 2);
        assert!(matches!(
            app.anki_note_type("xx".into(), false),
            Err(AllreadsError::Missing { .. })
        ));

        assert_eq!(app.anki_deck_id().unwrap(), None);
        app.set_anki_deck_id(12).unwrap();
        app.set_anki_model_id("fr".into(), 34).unwrap();
        let app = new_app(&dir);
        assert_eq!(app.anki_deck_id().unwrap(), Some(12));
        assert_eq!(app.anki_model_id("FR".into()).unwrap(), Some(34));
        assert_eq!(app.anki_model_id("de".into()).unwrap(), None);
    }

    fn statuses_of(chapter: &Chapter, key: &str) -> Vec<WordStatus> {
        chapter
            .blocks
            .iter()
            .flat_map(|b| &b.tokens)
            .filter(|t| t.term_key.as_deref() == Some(key))
            .filter_map(|t| t.status)
            .collect()
    }

    /// Step 24's check on the Rust side: a saved phrase is marked where it appears.
    #[test]
    fn a_saved_phrase_is_marked_wherever_it_appears() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let book = app
            .import_book(copy_in(&dir, "sample.txt"), "fr".into())
            .unwrap();
        let saved = app
            .save_term(input(
                "Premier  paragraphe",
                WordStatus::Learning { level: 2 },
            ))
            .unwrap();
        assert_eq!(saved.term_key, "premier paragraphe");
        let chapter = app.open_chapter(book.id, 0).unwrap();
        let marked: Vec<(&str, u32)> = chapter
            .blocks
            .iter()
            .flat_map(|b| &b.tokens)
            .filter_map(|t| t.phrase.as_ref().map(|p| (t.text.as_str(), p.id)))
            .collect();
        // "premier", the space, "paragraphe": one occurrence, one id.
        assert_eq!(marked, [("premier", 0), (" ", 0), ("paragraphe", 0)]);
        let mark = chapter.blocks[0]
            .tokens
            .iter()
            .find_map(|t| t.phrase.clone())
            .unwrap();
        assert_eq!(mark.term_key, "premier paragraphe");
        assert_eq!(mark.status, WordStatus::Learning { level: 2 });
        // The words keep their own status too.
        assert_eq!(statuses_of(&chapter, "premier"), [WordStatus::New]);
    }

    #[test]
    fn words_import_with_cleanup_status_rules_and_a_badge() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        assert_eq!(
            app.clean_import_word("fr".into(), "le chat (m.)".into())
                .unwrap()
                .as_deref(),
            Some("chat")
        );
        let row = |word: &str, meaning: Option<&str>, progress| ImportRow {
            word: word.into(),
            meaning: meaning.map(str::to_string),
            progress,
        };
        let rows = vec![
            row(
                "le <b>chat</b>",
                Some("cat<br>kitty"),
                Some(CardProgress::Interval { days: 30 }),
            ),
            row("chien", None, Some(CardProgress::Interval { days: 4 })),
            row("oiseau", None, Some(CardProgress::Suspended)),
            row("[sound:x.mp3]", None, None),
        ];
        let summary = app
            .import_words(
                "fr".into(),
                "Anki · Animaux".into(),
                true,
                rows.clone(),
                ImportStatus::FollowReviews,
            )
            .unwrap();
        assert_eq!((summary.added, summary.skipped), (2, 2));
        let chat = app.find_term("fr".into(), "chat".into()).unwrap().unwrap();
        assert_eq!(
            (
                chat.status,
                chat.translation.as_str(),
                chat.imported_from.as_str()
            ),
            (WordStatus::WellKnown, "cat; kitty", "Anki · Animaux")
        );
        assert_eq!(
            app.find_term("fr".into(), "chien".into())
                .unwrap()
                .unwrap()
                .status,
            WordStatus::Learning { level: 3 }
        );
        // Again with one level for all: nothing is downgraded.
        let again = app
            .import_words(
                "fr".into(),
                "Anki · Animaux".into(),
                true,
                rows,
                ImportStatus::Fixed {
                    status: WordStatus::Learning { level: 1 },
                },
            )
            .unwrap();
        assert_eq!(
            (again.added, again.upgraded, again.unchanged, again.skipped),
            (1, 0, 2, 1)
        );
        assert!(app
            .import_words(
                "fr".into(),
                "x".into(),
                false,
                Vec::new(),
                ImportStatus::Fixed {
                    status: WordStatus::New
                }
            )
            .is_err());
    }

    #[test]
    fn a_text_export_is_read_as_one_group() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let file = dir.path().join("upload");
        fs::write(
            &file,
            "#separator:tab\n#columns:Front\tBack\nle chat\tcat\n",
        )
        .unwrap();
        let groups = app
            .read_import_file(file.display().to_string(), "Vocab.txt".into())
            .unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(
            (groups[0].name.as_str(), groups[0].fields.len()),
            ("Vocab.txt", 2)
        );
        assert_eq!(groups[0].notes[0].fields, ["le chat", "cat"]);
        fs::write(&file, "").unwrap();
        assert!(app
            .read_import_file(file.display().to_string(), "x.csv".into())
            .is_err());
        assert!(app
            .read_import_file(file.display().to_string(), "x.apkg".into())
            .is_err());
    }

    #[test]
    fn a_capitalised_common_word_is_saved_and_translated_in_lower_case() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        // Without a dictionary, words stay as written.
        assert_eq!(
            app.save_term(input("Lire", WordStatus::Learning { level: 1 }))
                .unwrap()
                .display_text,
            "Lire"
        );
        let (gz, sum) = tiny_dictionary_download(dir.path());
        app.install_dictionary(gz, sum, "fr-en".into()).unwrap();
        let saved = app
            .save_term(input("Lisait", WordStatus::Learning { level: 1 }))
            .unwrap();
        assert_eq!(saved.display_text, "lisait");
        // Unknown to the dictionary: kept, as it may be a name.
        let saved = app
            .save_term(input("Gregor", WordStatus::Learning { level: 1 }))
            .unwrap();
        assert_eq!(saved.display_text, "Gregor");
        let lookup = app
            .lookup_word("fr".into(), "Pierre précieuse".into())
            .unwrap()
            .unwrap();
        assert_eq!(lookup.common_form, "pierre précieuse");
    }

    #[test]
    fn the_dictionary_suggests_phrases_around_a_word() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let words: Vec<String> = ["d'", "une", "Pierre", "précieuse"]
            .map(String::from)
            .to_vec();
        // No dictionary yet: no suggestions.
        assert!(app
            .phrase_suggestions("fr".into(), words.clone(), 2)
            .unwrap()
            .is_empty());
        let (gz, sum) = tiny_dictionary_download(dir.path());
        app.install_dictionary(gz, sum, "fr-en".into()).unwrap();
        let found = app.phrase_suggestions("fr".into(), words, 2).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].first_word, found[0].last_word), (2, 3));
        assert_eq!(found[0].gloss, "gemstone");
        assert_eq!(found[0].term_key, "pierre précieuse");
    }

    /// Step 13's check on the Rust side: saving a word recolours every occurrence.
    #[test]
    fn saving_a_word_recolours_every_occurrence_in_the_chapter() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let id = app
            .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
            .unwrap()
            .id;
        let before = app.open_chapter(id, 0).unwrap();
        assert_eq!(
            statuses_of(&before, "homme"),
            [WordStatus::New, WordStatus::New]
        );

        let saved = app
            .save_term(input("homme", WordStatus::Learning { level: 2 }))
            .unwrap();
        assert_eq!(saved.term_key, "homme");

        let after = app.open_chapter(id, 0).unwrap();
        let learning = WordStatus::Learning { level: 2 };
        assert_eq!(statuses_of(&after, "homme"), [learning, learning]);
    }

    #[test]
    fn find_term_returns_the_saved_translation_for_any_casing() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        app.save_term(input("Homme", WordStatus::Learning { level: 1 }))
            .unwrap();
        let found = app.find_term("fr".into(), "HOMME".into()).unwrap().unwrap();
        assert_eq!(
            (found.display_text.as_str(), found.translation.as_str()),
            ("Homme", "man")
        );
        assert_eq!(app.find_term("fr".into(), "femme".into()).unwrap(), None);
    }

    #[test]
    fn forgetting_a_word_makes_it_new_again() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        app.save_term(input("homme", WordStatus::Ignored)).unwrap();
        assert!(app.forget_term("fr".into(), "homme".into()).unwrap());
        assert!(!app.forget_term("fr".into(), "homme".into()).unwrap());
        assert_eq!(app.find_term("fr".into(), "homme".into()).unwrap(), None);
    }

    #[test]
    fn saving_with_new_or_a_bad_level_is_invalid_input() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        for status in [WordStatus::New, WordStatus::Learning { level: 7 }] {
            assert!(matches!(
                app.save_term(input("homme", status)),
                Err(AllreadsError::InvalidInput { .. })
            ));
        }
        assert!(matches!(
            app.save_term(input("   ", WordStatus::WellKnown)),
            Err(AllreadsError::InvalidInput { .. })
        ));
    }

    #[test]
    fn book_difficulty_follows_saved_words() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let id = app
            .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
            .unwrap()
            .id;
        let before = app.book_difficulty(id).unwrap();
        assert!(before.words > 0);
        assert_eq!(before.new_words, before.words);
        assert!((before.new_percent - 100.0).abs() < 1e-9);
        save_word(&app, "homme", Status::WellKnown);
        let after = app.book_difficulty(id).unwrap();
        assert_eq!(after.words, before.words);
        assert!(after.new_words < before.new_words);
        assert_eq!(after.unique_new_words + 1, before.unique_new_words);
    }

    #[test]
    fn word_progress_has_today_s_counts() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        save_word(&app, "chat", Status::Learning(1));
        let progress = app.word_progress("fr".into()).unwrap();
        assert_eq!(progress.days.len(), 1);
        assert_eq!((progress.days[0].known, progress.days[0].learning), (0, 1));
    }

    #[test]
    fn pasted_text_and_articles_become_txt_books_with_their_titles() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let article = app
            .prepare_article(
                "<html lang=fr><head><title>Le titre</title></head><body><article><p>Bonjour tout le monde.</p></article></body></html>"
                    .into(),
                Some("text/html; charset=utf-8".into()),
            )
            .unwrap();
        assert_eq!(
            (article.title.as_str(), article.language.as_deref()),
            ("Le titre", Some("fr"))
        );
        let book = app.import_book(article.path, "fr".into()).unwrap();
        assert_eq!(book.title, "Le titre");
        let text = app
            .prepare_text(
                None,
                "Il était une fois un renard qui errait.\n\nFin.".into(),
            )
            .unwrap();
        assert_eq!(text.title, "Il était une fois un renard");
        assert!(matches!(
            app.prepare_text(None, "  ".into()),
            Err(AllreadsError::InvalidInput { .. })
        ));
        assert!(matches!(
            app.prepare_article("<html></html>".into(), None),
            Err(AllreadsError::InvalidInput { .. })
        ));
        let picked = app.new_book_path(Some("Notes.txt".into()), "txt".into());
        assert!(picked.ends_with("books/Notes.txt"));
    }

    #[test]
    fn mark_rest_known_skips_earlier_blocks_and_saved_words() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let id = app
            .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
            .unwrap()
            .id;
        app.save_term(input("homme", WordStatus::Learning { level: 3 }))
            .unwrap();
        // Block 0 is the heading "Chapitre un"; start at block 1.
        let marked = app.mark_rest_known(id, 0, 1).unwrap();
        assert!(!marked.term_ids.is_empty());
        let chapter = app.open_chapter(id, 0).unwrap();
        assert_eq!(statuses_of(&chapter, "chapitre"), [WordStatus::New]);
        assert_eq!(statuses_of(&chapter, "lisait"), [WordStatus::WellKnown]);
        assert_eq!(
            statuses_of(&chapter, "homme"),
            [
                WordStatus::Learning { level: 3 },
                WordStatus::Learning { level: 3 }
            ]
        );
        assert!(app.mark_rest_known(id, 0, 1).unwrap().term_ids.is_empty());
        // Undo puts the words back to new.
        let count = marked.term_ids.len() as u32;
        assert_eq!(app.undo_mark_known(marked.term_ids).unwrap(), count);
        let chapter = app.open_chapter(id, 0).unwrap();
        assert_eq!(statuses_of(&chapter, "lisait"), [WordStatus::New]);
    }

    /// A DeepL client pointed at a local server that answers one request.
    fn mock_deepl(status: u16, body: &str) -> (DeepL, std::thread::JoinHandle<String>) {
        let (url, handle) = allreads_core::translate::mock_server::serve_once(status, body);
        (DeepL::with_base_url("test-key:fx", &url), handle)
    }

    /// Step 14's check on the Rust side: the second lookup is instant (cached).
    #[test]
    fn a_second_translation_comes_from_the_cache_without_a_request() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let (deepl, server) = mock_deepl(200, r#"{"translations":[{"text":"The man reads."}]}"#);
        let first = app
            .translate_with(&deepl, "L'homme  lit.", None, "fr", "en")
            .unwrap();
        assert_eq!(
            first,
            Translation {
                text: "The man reads.".into(),
                from_cache: false
            }
        );
        server.join().unwrap();
        // The mock server is gone: this only works if no request is made.
        let second = app
            .translate_with(&deepl, "L'homme lit.", None, "fr", "en")
            .unwrap();
        assert_eq!(
            second,
            Translation {
                text: "The man reads.".into(),
                from_cache: true
            }
        );
    }

    #[test]
    fn a_word_is_cached_per_sentence_it_was_translated_in() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let (deepl, server) = mock_deepl(200, r#"{"translations":[{"text":"reads"}]}"#);
        app.translate_with(&deepl, "lit", Some("Il lit."), "fr", "en")
            .unwrap();
        server.join().unwrap();
        let (deepl, server) = mock_deepl(200, r#"{"translations":[{"text":"bed"}]}"#);
        let bed = app
            .translate_with(&deepl, "lit", Some("Le lit."), "fr", "en")
            .unwrap();
        server.join().unwrap();
        assert_eq!(
            bed,
            Translation {
                text: "bed".into(),
                from_cache: false
            }
        );
        let offline = DeepL::with_base_url("k", "http://127.0.0.1:9");
        let reads = app
            .translate_with(&offline, "lit", Some("Il lit."), "fr", "en")
            .unwrap();
        assert_eq!(
            reads,
            Translation {
                text: "reads".into(),
                from_cache: true
            }
        );
    }

    #[test]
    fn translation_errors_are_typed_and_not_cached() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let (deepl, server) = mock_deepl(456, "{}");
        assert!(matches!(
            app.translate_with(&deepl, "chat", None, "fr", "en"),
            Err(AllreadsError::TranslationQuotaExceeded)
        ));
        server.join().unwrap();
        let (deepl, server) = mock_deepl(403, "{}");
        assert!(matches!(
            app.translate_with(&deepl, "chat", None, "fr", "en"),
            Err(AllreadsError::TranslationKeyRejected)
        ));
        server.join().unwrap();
    }

    #[test]
    fn being_offline_is_translation_unavailable() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let offline = DeepL::with_base_url("k", "http://127.0.0.1:9");
        assert!(matches!(
            app.translate_with(&offline, "chat", None, "fr", "en"),
            Err(AllreadsError::TranslationUnavailable { .. })
        ));
    }

    #[test]
    fn empty_text_or_key_is_invalid_input() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        assert!(matches!(
            app.translate("chat".into(), None, "fr".into(), "en".into(), "  ".into()),
            Err(AllreadsError::InvalidInput { .. })
        ));
        let offline = DeepL::with_base_url("k", "http://127.0.0.1:9");
        assert!(matches!(
            app.translate_with(&offline, "   ", None, "fr", "en"),
            Err(AllreadsError::InvalidInput { .. })
        ));
    }

    #[test]
    fn language_info_has_the_dictionary_url() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let french = app.language_info("fr".into()).unwrap().unwrap();
        assert_eq!(french.name, "French");
        assert!(french.dictionary_url.contains("{term}"));
        assert_eq!(french.tts_locale, "fr-FR");
        assert_eq!(app.language_info("zz".into()).unwrap(), None);
    }

    #[test]
    fn list_terms_returns_saved_words_of_one_language() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        app.save_term(input("Homme", WordStatus::Learning { level: 2 }))
            .unwrap();
        app.save_term(input("chat", WordStatus::WellKnown)).unwrap();
        let keys: Vec<String> = app
            .list_terms("fr".into())
            .unwrap()
            .into_iter()
            .map(|t| t.term_key)
            .collect();
        assert_eq!(keys, ["chat", "homme"]);
        assert!(app.list_terms("de".into()).unwrap().is_empty());
    }

    #[test]
    fn only_the_most_recently_used_books_stay_open() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let ids: Vec<i64> = (0..4)
            .map(|i| {
                let name = format!("tiny-{i}.epub");
                let path = dir.path().join(&name);
                fs::copy(fixture("tiny.epub"), &path).unwrap();
                app.import_book(path.display().to_string(), "fr".into())
                    .unwrap()
                    .id
            })
            .collect();
        let open: Vec<i64> = app
            .open_books
            .lock()
            .unwrap()
            .iter()
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(open, ids[1..]);
        // Reading the oldest book reopens it and makes it the most recent.
        app.open_chapter(ids[0], 0).unwrap();
        let open: Vec<i64> = app
            .open_books
            .lock()
            .unwrap()
            .iter()
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(open, [ids[2], ids[3], ids[0]]);
    }

    #[test]
    fn a_single_book_is_found_by_id() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let id = app
            .import_book(copy_in(&dir, "tiny.epub"), "fr".into())
            .unwrap()
            .id;
        assert_eq!(
            app.book(id).unwrap().map(|b| b.title),
            Some("Le Petit Test".to_string())
        );
        assert_eq!(app.book(id + 1).unwrap(), None);
    }

    #[test]
    fn a_stale_chapter_list_is_repaired_when_the_book_opens() {
        let dir = tempfile::tempdir().unwrap();
        let id = new_app(&dir)
            .import_book(copy_in(&dir, "layout.epub"), "fr".into())
            .unwrap()
            .id;
        // Simulate an import from before empty chapters were skipped.
        let app = new_app(&dir);
        {
            let db = app.db().unwrap();
            db.set_chapter_titles(id, &[None, None, None]).unwrap();
            db.store_chapter_cache(id, 0, PARSER_VERSION, b"stale")
                .unwrap();
            db.save_position(id, 2, 5).unwrap();
        }
        let chapter = app.open_chapter(id, 0).unwrap();
        assert_eq!(chapter.blocks[0].tokens[0].text, "Texte");
        let book = app.book(id).unwrap().unwrap();
        assert_eq!(book.chapter_count, 2);
        assert_eq!((book.chapter_index, book.paragraph_index), (0, 0));
    }

    #[test]
    fn a_key_that_reads_usage_reports_it() {
        let (url, server) = allreads_core::translate::mock_server::serve(&[(
            200,
            r#"{"character_count":12,"character_limit":500000}"#,
        )]);
        let status = check_with(&DeepL::with_base_url("k", &url)).unwrap();
        assert_eq!(status.characters_used, Some(12));
        assert_eq!(server.join().unwrap().len(), 1);
    }

    #[test]
    fn a_scoped_key_that_can_only_translate_still_works() {
        let (url, server) = allreads_core::translate::mock_server::serve(&[
            (403, r#"{"message":"Forbidden"}"#),
            (200, r#"{"translations":[{"text":"ok"}]}"#),
        ]);
        let status = check_with(&DeepL::with_base_url("k", &url)).unwrap();
        assert_eq!(
            status,
            DeepLKeyStatus {
                characters_used: None,
                character_limit: None
            }
        );
        let requests = server.join().unwrap();
        assert!(requests[1].starts_with("POST /v2/translate "));
    }

    #[test]
    fn a_key_refused_for_both_is_rejected() {
        let (url, server) = allreads_core::translate::mock_server::serve(&[
            (403, r#"{"message":"Forbidden"}"#),
            (403, r#"{"message":"Forbidden"}"#),
        ]);
        assert!(matches!(
            check_with(&DeepL::with_base_url("k", &url)),
            Err(AllreadsError::TranslationKeyRejected)
        ));
        server.join().unwrap();
    }

    #[test]
    fn an_empty_deepl_key_is_invalid_input() {
        assert!(matches!(
            check_deepl_key("  ".into()),
            Err(AllreadsError::InvalidInput { .. })
        ));
    }

    #[test]
    fn unknown_book_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        assert!(matches!(
            app.open_chapter(42, 0),
            Err(AllreadsError::Missing { .. })
        ));
        assert!(matches!(
            app.chapter_titles(42),
            Err(AllreadsError::Missing { .. })
        ));
        assert!(matches!(
            app.delete_book(42),
            Err(AllreadsError::Missing { .. })
        ));
    }

    #[test]
    fn importing_in_an_unknown_language_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let result = new_app(&dir).import_book(copy_in(&dir, "tiny.epub"), "zz".into());
        assert!(matches!(
            result,
            Err(AllreadsError::Missing { what }) if what == "language zz"
        ));
    }

    #[test]
    fn deleting_a_book_removes_its_cover_and_copied_file() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let path = copy_in(&dir, "layout.epub");
        let summary = app.import_book(path.clone(), "fr".into()).unwrap();
        let cover = summary.cover_path.clone().expect("layout has a cover");
        assert!(Path::new(&cover).is_file());
        app.delete_book(summary.id).unwrap();
        assert!(!Path::new(&cover).exists());
        assert!(!Path::new(&path).exists());
        assert!(app.library().unwrap().is_empty());
    }

    #[test]
    fn deleting_never_touches_a_file_outside_app_storage() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        let outside = fixture("tiny.epub");
        let id = app.import_book(outside.clone(), "fr".into()).unwrap().id;
        app.delete_book(id).unwrap();
        assert!(Path::new(&outside).is_file());
    }

    #[test]
    fn typed_errors_for_bad_files() {
        let dir = tempfile::tempdir().unwrap();
        let app = new_app(&dir);
        assert!(matches!(
            app.import_book(fixture("broken.epub"), "fr".into()),
            Err(AllreadsError::InvalidBook { .. })
        ));
        assert!(matches!(
            app.import_book("/no/such/book.epub".into(), "fr".into()),
            Err(AllreadsError::NotFound { .. })
        ));
        let pdf = dir.path().join("book.pdf");
        fs::write(&pdf, b"%PDF").unwrap();
        assert!(matches!(
            app.import_book(pdf.display().to_string(), "fr".into()),
            Err(AllreadsError::UnsupportedFile { .. })
        ));
    }

    #[test]
    fn book_meta_is_read_without_importing() {
        let epub = read_book_meta(fixture("layout.epub")).unwrap();
        assert_eq!(epub.author.as_deref(), Some("Jules Auteur"));
        assert_eq!(epub.language.as_deref(), Some("fr"));
        let txt = read_book_meta(fixture("sample.txt")).unwrap();
        assert_eq!((txt.title.as_deref(), txt.chapters), (Some("sample"), 1));
    }
}
