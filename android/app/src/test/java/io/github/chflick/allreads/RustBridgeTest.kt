package io.github.chflick.allreads

import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import uniffi.allreads_ffi.AllreadsException
import uniffi.allreads_ffi.App
import uniffi.allreads_ffi.TermInput
import uniffi.allreads_ffi.WordStatus
import uniffi.allreads_ffi.coreVersion
import uniffi.allreads_ffi.readBookMeta

/**
 * Calls the real Rust library from Kotlin on the JVM, through the generated
 * bindings and JNA. These cover the Kotlin side of steps 5, 6, 9 and 13
 * without a phone; taps and screens still need one.
 */
class RustBridgeTest {
    @get:Rule
    val temporaryFolder = TemporaryFolder()

    private val fixtures = File(System.getProperty("allreads.fixtures") ?: error("allreads.fixtures not set"))

    @Test
    fun coreVersionComesFromRustAndMatchesTheApp() {
        // CI builds add "-build.N" to the app's version name.
        assertEquals(BuildConfig.VERSION_NAME.substringBefore("-build."), coreVersion())
    }

    /** Step 6's check: an invalid path raises a typed Kotlin exception. */
    @Test
    fun aMissingFileIsATypedKotlinException() {
        try {
            readBookMeta("/no/such/book.epub")
            fail("expected AllreadsException.NotFound")
        } catch (e: AllreadsException.NotFound) {
            assertTrue(e.path, e.path.endsWith("book.epub"))
        }
    }

    @Test
    fun importOpenAndSaveAWordFromKotlin() {
        val book = File(temporaryFolder.root, "tiny.epub")
        File(fixtures, "tiny.epub").copyTo(book)
        App(temporaryFolder.root.absolutePath).use { app ->
            val summary = app.importBook(book.absolutePath, "fr")
            assertEquals("Le Petit Test", summary.title)
            assertEquals(2u, summary.chapterCount)

            app.saveTerm(
                TermInput(
                    languageCode = "fr",
                    text = "homme",
                    status = WordStatus.Learning(2u),
                    translation = "man",
                    notes = "",
                    lemma = null,
                    sentence = null,
                    sentenceTranslation = null,
                    sourceBook = null,
                    sourceChapter = null,
                ),
            )
            val pending = app.pendingAnkiNotes()
            assertEquals(1, pending.size)
            assertEquals("Allreads (French)", app.ankiNoteType("fr", false).name)
            val chapter = app.openChapter(summary.id, 0u)
            val statuses = chapter.blocks
                .flatMap { it.tokens }
                .filter { it.termKey == "homme" }
                .map { it.status }
            assertEquals(listOf(WordStatus.Learning(2u), WordStatus.Learning(2u)), statuses)
        }
    }
}
