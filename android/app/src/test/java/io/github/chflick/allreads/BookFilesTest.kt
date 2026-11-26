package io.github.chflick.allreads

import io.github.chflick.allreads.data.BookFiles
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class BookFilesTest {
    @Test
    fun extensionComesFromTheNameFirst() {
        assertEquals("epub", BookFiles.extensionFor("Le Comte.EPUB", null))
        assertEquals("txt", BookFiles.extensionFor("notes.txt", "application/octet-stream"))
    }

    @Test
    fun extensionFallsBackToTheMimeType() {
        assertEquals("epub", BookFiles.extensionFor("book", "application/epub+zip"))
        assertEquals("txt", BookFiles.extensionFor(null, "text/plain"))
        assertNull(BookFiles.extensionFor("scan.pdf", "application/pdf"))
    }

    @Test
    fun languageSuggestionMatchesRegionalCodes() {
        val available = listOf("de", "en", "es", "fr")
        assertEquals("fr", BookFiles.suggestLanguage("fr-FR", available))
        assertEquals("fr", BookFiles.suggestLanguage(" FR_ca ", available))
        assertEquals("de", BookFiles.suggestLanguage("ja", available))
        assertEquals("de", BookFiles.suggestLanguage(null, available))
        assertNull(BookFiles.suggestLanguage("fr", emptyList()))
    }
}
