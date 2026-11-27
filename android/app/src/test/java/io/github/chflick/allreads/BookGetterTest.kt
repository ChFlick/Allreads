package io.github.chflick.allreads

import io.github.chflick.allreads.data.Fetcher
import io.github.chflick.allreads.data.Repository
import io.github.chflick.allreads.ui.books.BookGetter
import java.io.File
import java.io.IOException
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import uniffi.allreads_ffi.BookDifficulty
import uniffi.allreads_ffi.CatalogBook

/** "Find a book"'s Get, with the real core and a download served from the test fixtures. */
class BookGetterTest {
    @get:Rule
    val temp = TemporaryFolder()

    private val fixtures = File(System.getProperty("allreads.fixtures") ?: error("allreads.fixtures not set"))

    /** Serves the fixture EPUB for any link, or fails like a lost connection. */
    private inner class FixtureFetcher(private val fail: Boolean = false) : Fetcher {
        val asked = mutableListOf<String>()
        override fun text(url: String) = error("not used")
        override fun download(url: String, target: File, progress: (Long) -> Unit) {
            asked += url
            if (fail) throw IOException("connection lost")
            File(fixtures, "tiny.epub").copyTo(target, overwrite = true)
        }
    }

    private val catalogBook = CatalogBook(
        id = 4650, title = "Candide", author = "Voltaire", subjects = emptyList(),
        url = "https://example.org/4650.epub", year = 1759, popularity = 1u,
        difficulty = BookDifficulty(1u, 1u, 0u, 1u, 1u, 100.0), worthLearning = 0u,
    )

    @Test
    fun getDownloadsTheBookIntoTheLibraryAndCleansUp() = runBlocking {
        val repository = Repository(temp.newFolder("data"))
        val cache = temp.newFolder("cache")
        val fetcher = FixtureFetcher()
        val summary = BookGetter(repository, fetcher, cache).get(catalogBook, "fr")

        assertEquals(listOf("https://example.org/4650.epub"), fetcher.asked)
        assertEquals("fr", summary.languageCode)
        assertEquals(listOf(summary.id), repository.library().map { it.id })
        // The copy lives in the books folder; nothing is left in the cache.
        assertTrue(repository.booksDir.listFiles().orEmpty().any { it.name.endsWith(".epub") })
        assertTrue(cache.listFiles().orEmpty().isEmpty())
    }

    @Test
    fun aFailedDownloadImportsNothingAndLeavesNoFile() = runBlocking {
        val repository = Repository(temp.newFolder("data"))
        val cache = temp.newFolder("cache")
        try {
            BookGetter(repository, FixtureFetcher(fail = true), cache).get(catalogBook, "fr")
            fail("expected the download to fail")
        } catch (e: IOException) {
            assertEquals("connection lost", e.message)
        }
        assertTrue(repository.library().isEmpty())
        assertFalse(cache.listFiles().orEmpty().any())
    }
}
