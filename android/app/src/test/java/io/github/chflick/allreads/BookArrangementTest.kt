package io.github.chflick.allreads

import io.github.chflick.allreads.ui.library.BookProgress
import io.github.chflick.allreads.data.BookSort
import io.github.chflick.allreads.ui.library.LibraryView
import io.github.chflick.allreads.ui.library.arrange
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.allreads_ffi.BookSummary

class BookArrangementTest {
    private fun book(id: Long, title: String, author: String?, language: String = "fr", read: Long? = null) =
        BookSummary(id, title, author, language, null, 10u, 0u, 0u, read)

    // In the core's order: read books first, then unread, newest first.
    private val books = listOf(
        book(1, "Fables", "Jean de La Fontaine", read = 200),
        book(3, "Die Verwandlung", "Franz Kafka", language = "de", read = 100),
        book(4, "Été", null),
        book(2, "Candide", "Voltaire"),
    )

    private fun titles(view: LibraryView) = arrange(books, view).map { it.title }

    @Test
    fun theDefaultKeepsTheCoreOrder() {
        assertEquals(listOf("Fables", "Die Verwandlung", "Été", "Candide"), titles(LibraryView()))
        assertTrue(LibraryView().isDefault)
    }

    @Test
    fun searchLooksAtTitleAndAuthorIgnoringCase() {
        assertEquals(listOf("Die Verwandlung"), titles(LibraryView(query = "kafka")))
        assertEquals(listOf("Candide"), titles(LibraryView(query = " CAND ")))
        assertFalse(LibraryView(query = "x").isDefault)
    }

    @Test
    fun filtersByLanguageAndProgress() {
        assertEquals(listOf("Die Verwandlung"), titles(LibraryView(language = "de")))
        assertEquals(listOf("Fables", "Die Verwandlung"), titles(LibraryView(progress = BookProgress.Reading)))
        assertEquals(listOf("Été", "Candide"), titles(LibraryView(progress = BookProgress.NotStarted)))
    }

    @Test
    fun sortsByTitleAuthorAndAdded() {
        // "Été" sorts as "Ete", between "Die" and "Fables".
        assertEquals(listOf("Candide", "Die Verwandlung", "Été", "Fables"), titles(LibraryView(sort = BookSort.Title)))
        // Authors in order, the book without one last.
        assertEquals(
            listOf("Die Verwandlung", "Fables", "Candide", "Été"),
            titles(LibraryView(sort = BookSort.Author)),
        )
        assertEquals(listOf("Été", "Die Verwandlung", "Candide", "Fables"), titles(LibraryView(sort = BookSort.Added)))
    }
}
