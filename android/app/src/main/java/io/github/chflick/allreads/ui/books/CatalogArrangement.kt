package io.github.chflick.allreads.ui.books

import androidx.annotation.StringRes
import io.github.chflick.allreads.R
import io.github.chflick.allreads.ui.AppText
import uniffi.allreads_ffi.CatalogBook

/** Up to this share of new words a book is a good level to learn from (the green dot). */
const val FITS_PERCENT = 5.0

/** Up to this share it is a stretch (the amber dot); beyond, hard going. */
const val STRETCH_PERCENT = 12.0

/** How a book compares with the saved words. */
enum class FitFilter(@StringRes private val labelId: Int) {
    FitsMe(R.string.fits_me),
    Stretch(R.string.a_stretch),
    ;

    val label: String get() = AppText.get(labelId)

    fun matches(book: CatalogBook): Boolean = when (this) {
        FitsMe -> book.difficulty.newPercent <= FITS_PERCENT
        Stretch -> book.difficulty.newPercent > FITS_PERCENT && book.difficulty.newPercent <= STRETCH_PERCENT
    }
}

/** A book's length by its running words. */
enum class BookLength(@StringRes private val labelId: Int, private val words: LongRange) {
    Short(R.string.length_short, 0L until 30_000L),
    Medium(R.string.length_medium, 30_000L until 100_000L),
    Long(R.string.length_long, 100_000L..kotlin.Long.MAX_VALUE),
    ;

    val label: String get() = AppText.get(labelId)

    fun matches(book: CatalogBook): Boolean = book.difficulty.words.toLong() in words
}

/** The orders "Find a book" offers. */
enum class CatalogSort(@StringRes private val labelId: Int) {
    BestFit(R.string.sort_best_fit),
    Popular(R.string.sort_popular),
    Shortest(R.string.sort_shortest),
    Title(R.string.sort_title),
    ;

    val label: String get() = AppText.get(labelId)
}

/** The search, filters and sort chosen in "Find a book". */
data class CatalogView(
    val query: String = "",
    val fit: FitFilter? = null,
    val length: BookLength? = null,
    val sort: CatalogSort = CatalogSort.BestFit,
)

/**
 * The books to show: those matching the search (`matches`, the ids the core
 * found; null when nothing is searched) and the filters, in the chosen order.
 */
fun arrange(books: List<CatalogBook>, view: CatalogView, matches: Set<Long>?): List<CatalogBook> {
    val shown = books.filter { book ->
        (matches == null || book.id in matches) &&
            (view.fit == null || view.fit.matches(book)) &&
            (view.length == null || view.length.matches(book))
    }
    return when (view.sort) {
        CatalogSort.BestFit -> shown.sortedWith(bestFit)
        CatalogSort.Popular -> shown.sortedByDescending { it.popularity }
        CatalogSort.Shortest -> shown.sortedBy { it.difficulty.words }
        CatalogSort.Title -> shown.sortedBy { it.title.lowercase() }
    }
}

/**
 * Books that fit come first, those that teach the most new words (met five
 * times or more) ahead; then the stretches and the hard ones, easiest first.
 * Within a band, popular books break ties.
 */
private val bestFit: Comparator<CatalogBook> = compareBy<CatalogBook> { band(it) }
    .thenBy { if (band(it) == 0) -it.worthLearning.toLong() else 0L }
    .thenBy { it.difficulty.newPercent }
    .thenByDescending { it.popularity }

private fun band(book: CatalogBook): Int = when {
    book.difficulty.newPercent <= FITS_PERCENT -> 0
    book.difficulty.newPercent <= STRETCH_PERCENT -> 1
    else -> 2
}
