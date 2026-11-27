package io.github.chflick.allreads.ui.library

import androidx.annotation.StringRes
import io.github.chflick.allreads.R
import io.github.chflick.allreads.ui.AppText
import io.github.chflick.allreads.data.BookSort
import java.text.Collator
import uniffi.allreads_ffi.BookSummary

/** Which books the library shows, besides the search. */
enum class BookProgress(@StringRes private val labelId: Int) {
    All(R.string.all),
    Reading(R.string.reading),
    NotStarted(R.string.not_started),
    ;

    val label: String get() = AppText.get(labelId)
}

/** The search, filters and sort chosen in the library. The default shows everything, recently read first. */
data class LibraryView(
    val query: String = "",
    val language: String? = null,
    val progress: BookProgress = BookProgress.All,
    val sort: BookSort = BookSort.Recent,
) {
    /** True when nothing narrows or reorders the list, so the Continue card makes sense. */
    val isDefault: Boolean get() = query.isBlank() && language == null && progress == BookProgress.All && sort == BookSort.Recent
}

/**
 * The books to show, in order. [books] comes from the core already sorted by
 * Recent (recently read first, then unread books, newest first).
 */
fun arrange(books: List<BookSummary>, view: LibraryView): List<BookSummary> {
    val query = view.query.trim()
    val shown = books.filter { book ->
        (query.isEmpty() || book.title.contains(query, ignoreCase = true) ||
            book.author?.contains(query, ignoreCase = true) == true) &&
            (view.language == null || book.languageCode == view.language) &&
            when (view.progress) {
                BookProgress.All -> true
                BookProgress.Reading -> book.lastReadAt != null
                BookProgress.NotStarted -> book.lastReadAt == null
            }
    }
    // Collator compares like a dictionary: "Été" sorts with "Ete", not after "Z".
    val collator = Collator.getInstance().apply { strength = Collator.PRIMARY }
    return when (view.sort) {
        BookSort.Recent -> shown
        BookSort.Added -> shown.sortedByDescending { it.id }
        BookSort.Title -> shown.sortedWith { a, b -> collator.compare(a.title, b.title) }
        // Books without an author go last.
        BookSort.Author -> shown.sortedWith(
            compareBy<BookSummary> { it.author == null }
                .thenComparator { a, b -> collator.compare(a.author.orEmpty(), b.author.orEmpty()) }
                .thenComparator { a, b -> collator.compare(a.title, b.title) },
        )
    }
}
