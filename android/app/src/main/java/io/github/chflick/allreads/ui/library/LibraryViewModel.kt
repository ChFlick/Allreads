package io.github.chflick.allreads.ui.library

import io.github.chflick.allreads.data.Dictionaries
import io.github.chflick.allreads.R
import io.github.chflick.allreads.ui.AppText
import io.github.chflick.allreads.data.BookSort
import android.content.ContentResolver
import android.net.Uri
import android.provider.OpenableColumns
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import io.github.chflick.allreads.data.BookFiles
import io.github.chflick.allreads.data.Repository
import io.github.chflick.allreads.data.SettingsStore
import io.github.chflick.allreads.data.Fetcher
import io.github.chflick.allreads.data.HttpFetcher
import io.github.chflick.allreads.data.SharedImport
import io.github.chflick.allreads.ui.describe
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.allreads_ffi.BookSummary
import uniffi.allreads_ffi.WordStatus
import uniffi.allreads_ffi.PreparedText
import uniffi.allreads_ffi.BookDifficulty

/** A picked file, copied into app storage and waiting for its language. */
data class PendingImport(
    val file: File,
    val title: String,
    val languages: List<String>,
    val suggested: String?,
)

data class LibraryState(
    val books: List<BookSummary> = emptyList(),
    val loading: Boolean = true,
    val pending: PendingImport? = null,
    val message: String? = null,
    /** The search, filters and sort; [shown] is the result. */
    val view: LibraryView = LibraryView(),
    /** Share of new words per book id, filled in the background after a refresh. */
    val difficulty: Map<Long, BookDifficulty> = emptyMap(),
    /** True while a web article is being fetched. */
    val fetching: Boolean = false,
    /** A language to offer the vocabulary check for, or null. */
    val vocabCheckOffer: String? = null,
    /** The getting-started steps once books are in, or null when they are all done or hidden. */
    val setup: SetupSteps? = null,
) {
    // Worked out once per state, not on every recomposition.
    val shown: List<BookSummary> by lazy { arrange(books, view) }
    /** The languages of the books, for the filter chips. */
    val languages: List<String> by lazy { books.map { it.languageCode }.distinct().sorted() }
}

/**
 * The first steps after the first book: what is done and what is left, so a
 * new reader still sees them once the empty library's page has gone.
 */
data class SetupSteps(
    val deeplKey: Boolean,
    val ankiDeck: Boolean,
    /** The language whose offline dictionary is offered, or null when none is published. */
    val dictionaryLanguage: String?,
    val dictionary: Boolean,
) {
    val allDone: Boolean get() = deeplKey && ankiDeck && (dictionaryLanguage == null || dictionary)
}

class LibraryViewModel(
    private val repository: Repository,
    private val settings: SettingsStore? = null,
    private val fetcher: Fetcher = HttpFetcher(),
    /** For offering the check when the dictionary is published but not installed yet. */
    private val dictionaries: Dictionaries? = null,
    /** Whether a DeepL key is saved, for the getting-started card. */
    private val hasDeeplKey: () -> Boolean = { false },
) : ViewModel() {
    private val savedSort = settings?.settings?.value?.librarySort ?: BookSort.Recent
    private val _state = MutableStateFlow(LibraryState(view = LibraryView(sort = savedSort)))
    val state: StateFlow<LibraryState> = _state.asStateFlow()

    init {
        refresh()
    }

    /** Changes the search or filters. A new sort is remembered for next time. */
    fun changeView(view: LibraryView) {
        if (view.sort != _state.value.view.sort) settings?.update { it.copy(librarySort = view.sort) }
        _state.update { it.copy(view = view) }
    }

    /** Counts books not counted yet, one at a time; one job for the whole view model. */
    private var counting: Job? = null

    /** Books whose count failed in this session, so they are not retried in a loop. */
    private val uncountable = mutableSetOf<Long>()

    /**
     * Reloads the list, with the difficulty of every book already counted in
     * one call, then counts the rest in the background.
     */
    fun refresh() {
        viewModelScope.launch {
            val books = runCatching { repository.library() }
            val counted = runCatching { repository.countedDifficulties() }.getOrDefault(emptyList())
            _state.update {
                it.copy(
                    books = books.getOrDefault(it.books),
                    difficulty = counted.associate { c -> c.bookId to c.difficulty },
                    loading = false,
                    message = books.exceptionOrNull()?.let(::describe) ?: it.message,
                )
            }
            if (counting?.isActive != true) counting = viewModelScope.launch { countMissing() }
            val list = books.getOrDefault(emptyList())
            val offer = runCatching { vocabCheckOffer(list) }.getOrNull()
            val setup = runCatching { setupSteps(list) }.getOrNull()
            _state.update { it.copy(vocabCheckOffer = offer, setup = setup) }
        }
    }

    /** The language the reader reads in: that of the book read last, or of most books before any was read. */
    private fun mainLanguage(books: List<BookSummary>): String? =
        books.filter { it.lastReadAt != null }.maxByOrNull { it.lastReadAt ?: 0 }?.languageCode
            ?: books.groupingBy { it.languageCode }.eachCount().maxByOrNull { it.value }?.key

    /** What is left of getting started; null without books (the empty library shows the steps) or once done or hidden. */
    private suspend fun setupSteps(books: List<BookSummary>): SetupSteps? {
        val current = settings?.settings?.value ?: return null
        if (books.isEmpty() || current.setupDismissed) return null
        val language = mainLanguage(books)
        // Usually fetched already, for the vocabulary check.
        dictionaries?.ensureListed()
        // Installed, or published.
        val pair = language?.let { code ->
            dictionaries?.list?.value?.installed?.firstOrNull { it.pair.startsWith("$code-") }?.pair
                ?: dictionaries?.availableFor(code)?.pair
        }
        val steps = SetupSteps(
            deeplKey = hasDeeplKey(),
            ankiDeck = runCatching { repository.ankiDeckId() }.getOrNull() != null,
            dictionaryLanguage = language?.takeIf { pair != null },
            dictionary = pair != null && dictionaries?.list?.value?.installed?.any { it.pair == pair } == true,
        )
        return steps.takeIf { !it.allDone }
    }

    /** Hides the getting-started card for good. */
    fun dismissSetup() {
        settings?.update { it.copy(setupDismissed = true) }
        _state.update { it.copy(setup = null) }
    }

    /**
     * The language to offer the vocabulary check for: that of the book read
     * last (or most books, before any was read), when its dictionary has word
     * frequencies, no check was finished there,
     * few words are known yet, and the card was not dismissed.
     */
    private suspend fun vocabCheckOffer(books: List<BookSummary>): String? {
        val current = settings?.settings?.value ?: return null
        if (current.vocabCheckDismissed) return null
        val language = mainLanguage(books) ?: return null
        if (language in current.vocabChecked) return null
        if (!repository.vocabularyCheckAvailable(language) && !publishedWithFrequencies(language)) return null
        if (repository.vocabularyCheckCount(language) > 0) return null
        val known = repository.listTerms(language).count { it.status == WordStatus.WellKnown }
        return language.takeIf { known < FEW_KNOWN_WORDS }
    }

    /** True when the language's published dictionary supports the check (it downloads first). */
    private suspend fun publishedWithFrequencies(language: String): Boolean {
        val dictionaries = dictionaries ?: return false
        dictionaries.ensureListed()
        return dictionaries.availableFor(language)?.frequencySource != null
    }

    /** Hides the vocabulary-check card for good (it stays in Settings). */
    fun dismissVocabCheck() {
        settings?.update { it.copy(vocabCheckDismissed = true) }
        _state.update { it.copy(vocabCheckOffer = null) }
    }

    private companion object {
        /** Below this many known words, the check is worth offering. */
        const val FEW_KNOWN_WORDS = 300
    }

    /** Counts the listed books without a difficulty, reading the list again after each, so new imports join in. */
    private suspend fun countMissing() {
        while (true) {
            val state = _state.value
            val book = state.books.firstOrNull { it.id !in state.difficulty && it.id !in uncountable } ?: return
            runCatching { repository.bookDifficulty(book.id) }
                .onSuccess { difficulty -> _state.update { it.copy(difficulty = it.difficulty + (book.id to difficulty)) } }
                .onFailure { uncountable += book.id }
        }
    }

    /** Step 1 of an import: copy the picked file in and read its metadata. */
    fun pick(resolver: ContentResolver, uri: Uri) {
        viewModelScope.launch {
            val result = runCatching {
                val (name, mime) = withContext(Dispatchers.IO) { describeUri(resolver, uri) }
                val extension = BookFiles.extensionFor(name, mime)
                    ?: error(AppText.get(R.string.only_epub_and_txt_books))
                // Named after the picked file, so a TXT book's title is its file name.
                val file = File(repository.newBookPath(name, extension))
                withContext(Dispatchers.IO) {
                    resolver.openInputStream(uri).use { input ->
                        requireNotNull(input) { AppText.get(R.string.the_file_could_not_be) }
                        file.outputStream().use { input.copyTo(it) }
                    }
                }
                try {
                    val meta = repository.readMeta(file.absolutePath)
                    val languages = repository.languageCodes()
                    PendingImport(
                        file = file,
                        title = meta.title ?: file.nameWithoutExtension,
                        languages = languages,
                        suggested = BookFiles.suggestLanguage(meta.language, languages),
                    )
                } catch (e: Exception) {
                    file.delete()
                    throw e
                }
            }
            _state.update {
                it.copy(pending = result.getOrNull(), message = result.exceptionOrNull()?.let(::describe))
            }
        }
    }

    /** A link or text shared to the app. */
    fun importShared(shared: SharedImport) = when (shared) {
        is SharedImport.Link -> importArticle(shared.url)
        is SharedImport.Text -> importText(shared.title, shared.text)
    }

    /** Fetches a web page and offers its article for import, like a picked TXT file. */
    fun importArticle(address: String) {
        val url = address.trim().let { if (it.startsWith("http://") || it.startsWith("https://")) it else "https://$it" }
        _state.update { it.copy(fetching = true) }
        viewModelScope.launch {
            val result = runCatching {
                val page = withContext(Dispatchers.IO) { fetcher.page(url) }
                pending(repository.prepareArticle(page.bytes, page.contentType))
            }
            _state.update {
                it.copy(
                    fetching = false,
                    pending = result.getOrNull(),
                    message = result.exceptionOrNull()?.let(::describe),
                )
            }
        }
    }

    /** Offers pasted or shared text for import. Without a title, the core takes its first words. */
    fun importText(title: String?, text: String) {
        viewModelScope.launch {
            val result = runCatching { pending(repository.prepareText(title, text)) }
            _state.update { it.copy(pending = result.getOrNull(), message = result.exceptionOrNull()?.let(::describe)) }
        }
    }

    /** Text the core has written as a TXT book, waiting for the language dialog. */
    private suspend fun pending(prepared: PreparedText): PendingImport {
        val languages = repository.languageCodes()
        return PendingImport(
            file = File(prepared.path),
            title = prepared.title,
            languages = languages,
            suggested = BookFiles.suggestLanguage(prepared.language, languages),
        )
    }

    /** Step 2 of an import: store it in the chosen language. */
    fun confirmImport(languageCode: String) {
        val pending = _state.value.pending ?: return
        _state.update { it.copy(pending = null) }
        viewModelScope.launch {
            val result = runCatching { repository.importBook(pending.file.absolutePath, languageCode) }
            if (result.isFailure) withContext(Dispatchers.IO) { pending.file.delete() }
            _state.update { it.copy(message = result.exceptionOrNull()?.let(::describe)) }
            refresh()
        }
    }

    fun cancelImport() {
        val pending = _state.value.pending ?: return
        _state.update { it.copy(pending = null) }
        viewModelScope.launch(Dispatchers.IO) { pending.file.delete() }
    }

    fun delete(book: BookSummary) {
        viewModelScope.launch {
            val result = runCatching { repository.deleteBook(book.id) }
            _state.update { it.copy(message = result.exceptionOrNull()?.let(::describe)) }
            refresh()
        }
    }

    fun messageShown() = _state.update { it.copy(message = null) }

    private fun describeUri(resolver: ContentResolver, uri: Uri): Pair<String?, String?> {
        val name = resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
            ?.use { cursor -> if (cursor.moveToFirst()) cursor.getString(0) else null }
        return name to resolver.getType(uri)
    }
}
