package io.github.chflick.allreads.ui.library

import android.content.ContentResolver
import android.net.Uri
import android.provider.OpenableColumns
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import io.github.chflick.allreads.data.BookFiles
import io.github.chflick.allreads.data.Repository
import io.github.chflick.allreads.data.SettingsStore
import io.github.chflick.allreads.ui.describe
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.allreads_ffi.BookSummary

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
) {
    val shown: List<BookSummary> get() = arrange(books, view)
    /** The languages of the books, for the filter chips. */
    val languages: List<String> get() = books.map { it.languageCode }.distinct().sorted()
}

class LibraryViewModel(
    private val repository: Repository,
    private val settings: SettingsStore? = null,
) : ViewModel() {
    private val savedSort = settings?.settings?.value?.librarySort
        ?.let { name -> BookSort.entries.firstOrNull { it.name == name } } ?: BookSort.Recent
    private val _state = MutableStateFlow(LibraryState(view = LibraryView(sort = savedSort)))
    val state: StateFlow<LibraryState> = _state.asStateFlow()

    init {
        refresh()
    }

    /** Changes the search or filters. A new sort is remembered for next time. */
    fun changeView(view: LibraryView) {
        if (view.sort != _state.value.view.sort) settings?.update { it.copy(librarySort = view.sort.name) }
        _state.update { it.copy(view = view) }
    }

    fun refresh() {
        viewModelScope.launch {
            val books = runCatching { repository.library() }
            _state.update {
                it.copy(
                    books = books.getOrDefault(it.books),
                    loading = false,
                    message = books.exceptionOrNull()?.let(::describe) ?: it.message,
                )
            }
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
                    PendingImport(
                        file = file,
                        title = meta.title ?: name ?: file.name,
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
