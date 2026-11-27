package io.github.chflick.allreads.ui.books

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import io.github.chflick.allreads.R
import io.github.chflick.allreads.data.BookCatalog
import io.github.chflick.allreads.data.Fetcher
import io.github.chflick.allreads.ui.AppText
import io.github.chflick.allreads.ui.describe
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.allreads_ffi.BookSummary
import uniffi.allreads_ffi.CatalogBook
import uniffi.allreads_ffi.CatalogWord

data class FindBooksState(
    /** The languages with an installed book list. */
    val languages: List<String> = emptyList(),
    val language: String? = null,
    val loading: Boolean = true,
    val books: List<CatalogBook> = emptyList(),
    val view: CatalogView = CatalogView(),
    /** The ids matching the search; null when nothing is searched. */
    val matches: Set<Long>? = null,
    /** The book whose details are open. */
    val selected: CatalogBook? = null,
    val newWords: List<CatalogWord> = emptyList(),
    /** Books being downloaded and imported. */
    val getting: Set<Long> = emptySet(),
    /** Books imported here, catalog id → library book id, for "Open". */
    val imported: Map<Long, Long> = emptyMap(),
    val message: String? = null,
)

/**
 * "Find a book": the installed book lists, compared with the saved words. Get
 * downloads a book's EPUB and imports it like a picked file.
 */
class FindBooksViewModel(
    private val catalog: BookCatalog,
    private val fetcher: Fetcher,
    private val cacheDir: File,
    /** The languages with an installed book list, the one read most recently first. */
    private val installedLanguages: suspend () -> List<String>,
) : ViewModel() {
    private val _state = MutableStateFlow(FindBooksState())
    val state: StateFlow<FindBooksState> = _state.asStateFlow()

    private var searchJob: Job? = null
    private val getter = BookGetter(catalog, fetcher, cacheDir)

    init {
        reload()
    }

    /** Reads the installed book lists again, e.g. after one was downloaded. */
    fun reload() {
        viewModelScope.launch {
            val languages = runCatching { installedLanguages() }.getOrDefault(emptyList())
            val language = _state.value.language?.takeIf { it in languages } ?: languages.firstOrNull()
            _state.update { it.copy(languages = languages, language = language) }
            load(language)
        }
    }

    fun chooseLanguage(code: String) {
        if (code == _state.value.language) return
        _state.update { it.copy(language = code, matches = null, view = it.view.copy(query = "")) }
        viewModelScope.launch { load(code) }
    }

    private suspend fun load(language: String?) {
        if (language == null) {
            _state.update { it.copy(loading = false, books = emptyList()) }
            return
        }
        _state.update { it.copy(loading = true) }
        val books = runCatching { catalog.catalogBooks(language) }
        _state.update {
            it.copy(
                loading = false,
                books = books.getOrDefault(emptyList()),
                message = books.exceptionOrNull()?.let(::describe),
            )
        }
    }

    fun setView(view: CatalogView) {
        val previous = _state.value.view
        _state.update { it.copy(view = view) }
        if (view.query != previous.query) search(view.query)
    }

    /** Asks the core for matches once typing pauses; a blank query shows everything. */
    private fun search(query: String) {
        searchJob?.cancel()
        val language = _state.value.language ?: return
        if (query.isBlank()) {
            _state.update { it.copy(matches = null) }
            return
        }
        searchJob = viewModelScope.launch {
            delay(SEARCH_PAUSE_MS)
            val ids = runCatching { catalog.searchCatalog(language, query) }.getOrDefault(emptyList())
            _state.update { if (it.view.query == query) it.copy(matches = ids.toSet()) else it }
        }
    }

    /** Opens a book's details with the new words it would teach. */
    fun select(book: CatalogBook?) {
        _state.update { it.copy(selected = book, newWords = emptyList()) }
        val language = _state.value.language ?: return
        book ?: return
        viewModelScope.launch {
            val words = runCatching { catalog.catalogNewWords(language, book.id, NEW_WORDS) }.getOrDefault(emptyList())
            _state.update { if (it.selected?.id == book.id) it.copy(newWords = words) else it }
        }
    }

    /** Downloads the book's EPUB into the books folder and imports it. */
    fun get(book: CatalogBook) {
        val language = _state.value.language ?: return
        if (book.id in _state.value.getting || book.id in _state.value.imported) return
        _state.update { it.copy(getting = it.getting + book.id) }
        viewModelScope.launch {
            val result = runCatching { getter.get(book, language) }
            _state.update { state ->
                result.fold(
                    onSuccess = { summary ->
                        state.copy(
                            getting = state.getting - book.id,
                            imported = state.imported + (book.id to summary.id),
                            message = AppText.get(R.string.added_to_your_library, summary.title),
                        )
                    },
                    onFailure = { e -> state.copy(getting = state.getting - book.id, message = describe(e)) },
                )
            }
        }
    }

    fun messageShown() = _state.update { it.copy(message = null) }

    private companion object {
        const val SEARCH_PAUSE_MS = 200L
        const val NEW_WORDS = 20
    }
}

/**
 * Get: downloads a catalog book's EPUB, copies it into the books folder and
 * imports it like a picked file. Apart from the view model, so JVM tests can
 * run it against the real core.
 */
class BookGetter(private val catalog: BookCatalog, private val fetcher: Fetcher, private val cacheDir: File) {
    suspend fun get(book: CatalogBook, languageCode: String): BookSummary {
        val download = File(cacheDir, "catalog-book-${book.id}.epub.download")
        try {
            withContext(Dispatchers.IO) { fetcher.download(book.url, download) {} }
            val path = catalog.newBookPath(book.title, "epub")
            withContext(Dispatchers.IO) { download.copyTo(File(path), overwrite = true) }
            return catalog.importBook(path, languageCode)
        } finally {
            download.delete()
        }
    }
}
