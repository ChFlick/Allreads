package io.github.chflick.allreads.ui.library

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
import uniffi.allreads_ffi.BookDifficulty

/** A picked file, copied into app storage and waiting for its language. */
data class PendingImport(
    val file: File,
    val title: String,
    val languages: List<String>,
    val suggested: String?,
    /** Stored as the book's title; for TXT files and web articles, whose copies have random names. */
    val titleOverride: String? = null,
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
) {
    // Worked out once per state, not on every recomposition.
    val shown: List<BookSummary> by lazy { arrange(books, view) }
    /** The languages of the books, for the filter chips. */
    val languages: List<String> by lazy { books.map { it.languageCode }.distinct().sorted() }
}

class LibraryViewModel(
    private val repository: Repository,
    private val settings: SettingsStore? = null,
    private val fetcher: Fetcher = HttpFetcher(),
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
        }
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
                    ?: error("Only EPUB and TXT books can be imported")
                val file = File(repository.booksDir, BookFiles.copyName(extension))
                withContext(Dispatchers.IO) {
                    resolver.openInputStream(uri).use { input ->
                        requireNotNull(input) { "The file could not be opened" }
                        file.outputStream().use { input.copyTo(it) }
                    }
                }
                try {
                    val meta = repository.readMeta(file.absolutePath)
                    val languages = repository.languageCodes()
                    // A TXT file has no title inside; its copy is named by a random id.
                    val fileTitle = name?.substringBeforeLast('.')?.takeIf { extension == "txt" && it.isNotBlank() }
                    PendingImport(
                        file = file,
                        title = fileTitle ?: meta.title ?: name ?: file.name,
                        languages = languages,
                        suggested = BookFiles.suggestLanguage(meta.language, languages),
                        titleOverride = fileTitle,
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
                val html = withContext(Dispatchers.IO) { fetcher.text(url) }
                val article = repository.articleFromHtml(html)
                if (article.paragraphs.isEmpty()) error("No readable text was found on that page")
                pendingText(article.title, article.paragraphs.joinToString("\n\n"), article.language)
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

    /** Offers pasted or shared text for import. Without a title, its first words become one. */
    fun importText(title: String?, text: String) {
        viewModelScope.launch {
            val name = title?.trim()?.takeIf { it.isNotEmpty() }
                ?: text.trim().split(Regex("\\s+")).take(6).joinToString(" ").let { if (it.length > 60) it.take(60) + "…" else it }
            val result = runCatching { pendingText(name, text, null) }
            _state.update { it.copy(pending = result.getOrNull(), message = result.exceptionOrNull()?.let(::describe)) }
        }
    }

    /** Writes text as a TXT copy in app storage, ready for the language dialog. */
    private suspend fun pendingText(title: String, text: String, language: String?): PendingImport {
        val file = File(repository.booksDir, BookFiles.copyName("txt"))
        withContext(Dispatchers.IO) { file.writeText(text) }
        val languages = repository.languageCodes()
        return PendingImport(
            file = file,
            title = title,
            languages = languages,
            suggested = BookFiles.suggestLanguage(language, languages),
            titleOverride = title,
        )
    }

    /** Step 2 of an import: store it in the chosen language. */
    fun confirmImport(languageCode: String) {
        val pending = _state.value.pending ?: return
        _state.update { it.copy(pending = null) }
        viewModelScope.launch {
            val result = runCatching { repository.importBook(pending.file.absolutePath, languageCode, pending.titleOverride) }
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
