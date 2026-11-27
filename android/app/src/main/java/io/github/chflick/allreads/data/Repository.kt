package io.github.chflick.allreads.data

import io.github.chflick.allreads.anki.NoteTypeStore
import io.github.chflick.allreads.anki.SyncStore
import uniffi.allreads_ffi.MarkedKnown
import uniffi.allreads_ffi.BookDifficulty
import uniffi.allreads_ffi.ImportGroup
import uniffi.allreads_ffi.ImportRow
import uniffi.allreads_ffi.ImportStatus
import uniffi.allreads_ffi.ImportSummary
import uniffi.allreads_ffi.BookDifficultyOf
import uniffi.allreads_ffi.WordProgress
import uniffi.allreads_ffi.PreparedText
import uniffi.allreads_ffi.WordStatus
import uniffi.allreads_ffi.AnkiNote
import uniffi.allreads_ffi.AvailableCatalog
import uniffi.allreads_ffi.AvailableDictionary
import uniffi.allreads_ffi.CatalogBook
import uniffi.allreads_ffi.CatalogWord
import uniffi.allreads_ffi.InstalledCatalog
import uniffi.allreads_ffi.DictionaryLookup
import uniffi.allreads_ffi.InstalledDictionary
import uniffi.allreads_ffi.PhraseSuggestion
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.allreads_ffi.AnkiNoteType
import uniffi.allreads_ffi.App
import uniffi.allreads_ffi.BookMeta
import uniffi.allreads_ffi.BookSummary
import uniffi.allreads_ffi.Chapter
import uniffi.allreads_ffi.DeepLKeyStatus
import uniffi.allreads_ffi.LanguageInfo
import uniffi.allreads_ffi.SavedTerm
import uniffi.allreads_ffi.TermInput
import uniffi.allreads_ffi.Translation
import uniffi.allreads_ffi.readBookMeta

/**
 * The app's only way into Rust. Every call runs on [Dispatchers.IO], so
 * screens never block the main thread. The Rust library (database, migrations,
 * presets) is opened on the first call, which is also on [Dispatchers.IO].
 */
class Repository(val dataDir: File) : NoteTypeStore, SyncStore, DictionaryFiles, CatalogFiles, BookCatalog {
    private val app: App by lazy { App(dataDir.absolutePath) }

    /** Where imported book files are copied. */
    val booksDir: File get() = File(dataDir, "books").apply { mkdirs() }

    suspend fun library(): List<BookSummary> = io { app.library() }

    suspend fun book(bookId: Long): BookSummary? = io { app.book(bookId) }

    suspend fun languageCodes(): List<String> = io { app.languageCodes() }

    suspend fun readMeta(path: String): BookMeta = io { readBookMeta(path) }

    override suspend fun importBook(path: String, languageCode: String): BookSummary =
        io { app.importBook(path, languageCode) }

    /** A free path in the books folder named after a picked file (`name`), for its copy. */
    override suspend fun newBookPath(name: String?, extension: String): String = io { app.newBookPath(name, extension) }

    /** Writes pasted or shared text as a TXT book; its first words title it if `title` is null. */
    suspend fun prepareText(title: String?, text: String): PreparedText = io { app.prepareText(title, text) }

    /** Decodes a fetched page, picks out its article and writes it as a TXT book. */
    suspend fun prepareArticle(page: ByteArray, contentType: String?): PreparedText =
        io { app.prepareArticle(page, contentType) }

    suspend fun bookDifficulty(bookId: Long): BookDifficulty = io { app.bookDifficulty(bookId) }

    /** Counts a book's words if needed; true if it counted now. */
    suspend fun ensureBookCounted(bookId: Long): Boolean = io { app.ensureBookCounted(bookId) }

    suspend fun wordLanguageCodes(): List<String> = io { app.wordLanguageCodes() }

    /** The word an import would take from a field ("le chat (m.)" → "chat"), for samples. */
    suspend fun cleanImportWord(languageCode: String, raw: String): String? = io { app.cleanImportWord(languageCode, raw) }

    /** An `.apkg` deck or a text/CSV export, read into groups of notes. */
    suspend fun readImportFile(path: String, name: String): List<ImportGroup> = io { app.readImportFile(path, name) }

    suspend fun importWords(
        languageCode: String,
        source: String,
        fromAnki: Boolean,
        rows: List<ImportRow>,
        status: ImportStatus,
    ): ImportSummary = io { app.importWords(languageCode, source, fromAnki, rows, status) }

    suspend fun countedDifficulties(): List<BookDifficultyOf> = io { app.countedDifficulties() }

    /** Changes only a saved word's status; returns the old one, or null if the word is not saved. */
    suspend fun setTermStatus(languageCode: String, text: String, status: WordStatus): WordStatus? =
        io { app.setTermStatus(languageCode, text, status) }

    suspend fun wordProgress(languageCode: String): WordProgress = io { app.wordProgress(languageCode) }



    suspend fun deleteBook(bookId: Long) = io { app.deleteBook(bookId) }

    suspend fun chapterTitles(bookId: Long): List<String?> = io { app.chapterTitles(bookId) }

    suspend fun openChapter(bookId: Long, index: UInt): Chapter = io { app.openChapter(bookId, index) }

    suspend fun savePosition(bookId: Long, chapter: UInt, block: UInt) =
        io { app.savePosition(bookId, chapter, block) }

    suspend fun findTerm(languageCode: String, text: String): SavedTerm? =
        io { app.findTerm(languageCode, text) }

    suspend fun saveTerm(input: TermInput): SavedTerm = io { app.saveTerm(input) }

    suspend fun forgetTerm(languageCode: String, text: String): Boolean =
        io { app.forgetTerm(languageCode, text) }

    /** Brings back a word just forgotten, whole; false when there is nothing to bring back. */
    suspend fun undoForget(languageCode: String, text: String): Boolean =
        io { app.undoForget(languageCode, text) }

    suspend fun languageInfo(languageCode: String): LanguageInfo? = io { app.languageInfo(languageCode) }

    suspend fun listTerms(languageCode: String): List<SavedTerm> = io { app.listTerms(languageCode) }

    /** Translates with DeepL; `context` is the sentence around a word. Repeats come from the cache. */
    suspend fun translate(
        text: String,
        context: String?,
        sourceLanguage: String,
        targetLanguage: String,
        deeplKey: String,
    ): Translation = io { app.translate(text, context, sourceLanguage, targetLanguage, deeplKey) }

    /** Checks a DeepL key by reading its usage, which costs no quota. */
    suspend fun checkDeeplKey(key: String): DeepLKeyStatus = io { uniffi.allreads_ffi.checkDeeplKey(key) }

    suspend fun markRestKnown(bookId: Long, chapter: UInt, fromBlock: UInt): MarkedKnown =
        io { app.markRestKnown(bookId, chapter, fromBlock) }

    suspend fun undoMarkKnown(termIds: List<Long>): UInt = io { app.undoMarkKnown(termIds) }

    /** The AnkiDroid deck chosen in Settings, if any. */
    suspend fun ankiDeckId(): Long? = io { app.ankiDeckId() }

    override suspend fun deckId(): Long? = ankiDeckId()

    override suspend fun pendingNotes(): List<AnkiNote> = io { app.pendingAnkiNotes() }

    override suspend fun waitingCount(): Int = io { app.pendingAnkiCount().toInt() }

    override suspend fun recordPush(note: AnkiNote, noteId: Long, deckId: Long) =
        io { app.recordAnkiPush(note.termId, note.queueId, noteId, deckId) }

    override suspend fun recordFailure(note: AnkiNote) = io { app.recordAnkiFailure(note.termId) }

    override suspend fun pendingSuspends(minAgeSeconds: Int): List<Long> = io { app.pendingAnkiSuspends(minAgeSeconds.toUInt()) }

    override suspend fun recordSuspended(noteId: Long) = io { app.recordAnkiSuspended(noteId) }

    suspend fun setAnkiDeckId(deckId: Long) = io { app.setAnkiDeckId(deckId) }

    override suspend fun modelId(languageCode: String): Long? = io { app.ankiModelId(languageCode) }

    override suspend fun setModelId(languageCode: String, modelId: Long) =
        io { app.setAnkiModelId(languageCode, modelId) }

    override suspend fun noteType(languageCode: String, includeWordCard: Boolean): AnkiNoteType =
        io { app.ankiNoteType(languageCode, includeWordCard) }

    override suspend fun installed(): List<InstalledDictionary> = io { app.installedDictionaries() }

    override suspend fun install(download: File, sha256: String, pair: String): InstalledDictionary =
        io { app.installDictionary(download.absolutePath, sha256, pair) }

    override suspend fun remove(pair: String) {
        io { app.removeDictionary(pair) }
    }

    override fun parseManifest(json: String): List<AvailableDictionary> =
        uniffi.allreads_ffi.parseDictionaryManifest(json)

    /** The offline dictionary's entry for a word; `null` if no dictionary is installed for its language. */
    suspend fun lookupWord(languageCode: String, text: String): DictionaryLookup? =
        io { app.lookupWord(languageCode, text) }

    suspend fun phraseSuggestions(languageCode: String, words: List<String>, index: UInt): List<PhraseSuggestion> =
        io { app.phraseSuggestions(languageCode, words, index) }

    override suspend fun installedCatalogs(): List<InstalledCatalog> = io { app.installedCatalogs() }

    override suspend fun installCatalog(download: File, sha256: String, languageCode: String): InstalledCatalog =
        io { app.installCatalog(download.absolutePath, sha256, languageCode) }

    override suspend fun removeCatalog(languageCode: String) {
        io { app.removeCatalog(languageCode) }
    }

    override fun parseCatalogManifest(json: String): List<AvailableCatalog> =
        uniffi.allreads_ffi.parseCatalogManifest(json)

    override suspend fun catalogBooks(languageCode: String): List<CatalogBook> = io { app.catalogBooks(languageCode) }

    override suspend fun searchCatalog(languageCode: String, query: String): List<Long> =
        io { app.searchCatalog(languageCode, query) }

    override suspend fun catalogNewWords(languageCode: String, bookId: Long, limit: Int): List<CatalogWord> =
        io { app.catalogNewWords(languageCode, bookId, limit.toUInt()) }

    private suspend fun <T> io(block: () -> T): T = withContext(Dispatchers.IO) { block() }

}
