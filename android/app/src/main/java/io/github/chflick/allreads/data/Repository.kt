package io.github.chflick.allreads.data

import io.github.chflick.allreads.anki.NoteTypeStore
import io.github.chflick.allreads.anki.SyncStore
import uniffi.allreads_ffi.MarkedKnown
import uniffi.allreads_ffi.AnkiNote
import uniffi.allreads_ffi.AvailableDictionary
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
class Repository(val dataDir: File) : NoteTypeStore, SyncStore, DictionaryFiles {
    private val app: App by lazy { App(dataDir.absolutePath) }

    /** Where imported book files are copied. */
    val booksDir: File get() = File(dataDir, "books").apply { mkdirs() }

    suspend fun library(): List<BookSummary> = io { app.library() }

    suspend fun book(bookId: Long): BookSummary? = io { app.book(bookId) }

    suspend fun languageCodes(): List<String> = io { app.languageCodes() }

    suspend fun readMeta(path: String): BookMeta = io { readBookMeta(path) }

    suspend fun importBook(path: String, languageCode: String): BookSummary =
        io { app.importBook(path, languageCode) }

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

    private suspend fun <T> io(block: () -> T): T = withContext(Dispatchers.IO) { block() }

}
