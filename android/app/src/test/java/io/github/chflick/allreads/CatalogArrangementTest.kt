package io.github.chflick.allreads

import io.github.chflick.allreads.ui.books.BookLength
import io.github.chflick.allreads.ui.books.CatalogSort
import io.github.chflick.allreads.ui.books.CatalogView
import io.github.chflick.allreads.ui.books.FitFilter
import io.github.chflick.allreads.ui.books.arrange
import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.allreads_ffi.BookDifficulty
import uniffi.allreads_ffi.CatalogBook

class CatalogArrangementTest {
    private fun book(id: Long, title: String, percent: Double, worth: Int = 0, words: Long = 50_000, popularity: Int = 0) =
        CatalogBook(
            id = id, title = title, author = "", subjects = emptyList(), url = "https://example.org/$id.epub",
            year = null, popularity = popularity.toUInt(),
            difficulty = BookDifficulty(
                words = words.toULong(), newWords = (words * percent / 100).toLong().toULong(), learningWords = 0u,
                uniqueWords = 1u, uniqueNewWords = 1u, newPercent = percent,
            ),
            worthLearning = worth.toUInt(),
        )

    private val easyFewNew = book(1, "Easy, little to learn", 2.0, worth = 3)
    private val easyMuchNew = book(2, "Easy, much to learn", 4.5, worth = 40)
    private val stretch = book(3, "Stretch", 9.0, worth = 200)
    private val hard = book(4, "Hard", 20.0, worth = 500, words = 150_000)
    private val shortHard = book(5, "Short and hard", 15.0, words = 12_000, popularity = 900)
    private val all = listOf(hard, stretch, easyFewNew, shortHard, easyMuchNew)

    private fun ids(view: CatalogView, matches: Set<Long>? = null) = arrange(all, view, matches).map { it.id }

    @Test
    fun bestFitPutsFittingBooksFirstThoseTeachingMostAheadThenTheEasiest() {
        assertEquals(listOf(2L, 1L, 3L, 5L, 4L), ids(CatalogView()))
    }

    @Test
    fun fitAndLengthFiltersNarrowTheList() {
        assertEquals(listOf(2L, 1L), ids(CatalogView(fit = FitFilter.FitsMe)))
        assertEquals(listOf(3L), ids(CatalogView(fit = FitFilter.Stretch)))
        assertEquals(listOf(5L), ids(CatalogView(length = BookLength.Short)))
        assertEquals(listOf(4L), ids(CatalogView(length = BookLength.Long)))
        assertEquals(emptyList<Long>(), ids(CatalogView(fit = FitFilter.FitsMe, length = BookLength.Short)))
    }

    @Test
    fun searchMatchesFromTheCoreKeepTheChosenOrder() {
        assertEquals(listOf(3L, 4L), ids(CatalogView(), matches = setOf(4L, 3L)))
        assertEquals(emptyList<Long>(), ids(CatalogView(), matches = emptySet()))
    }

    @Test
    fun otherSorts() {
        assertEquals(5L, ids(CatalogView(sort = CatalogSort.Popular)).first())
        assertEquals(5L, ids(CatalogView(sort = CatalogSort.Shortest)).first())
        assertEquals(listOf(1L, 2L, 4L, 5L, 3L), ids(CatalogView(sort = CatalogSort.Title)))
    }
}
