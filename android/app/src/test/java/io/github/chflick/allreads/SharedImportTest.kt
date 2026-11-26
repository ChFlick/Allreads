package io.github.chflick.allreads

import io.github.chflick.allreads.data.SharedImport
import org.junit.Assert.assertEquals
import org.junit.Test

class SharedImportTest {
    @Test
    fun aBrowserShareIsALink() {
        assertEquals(
            SharedImport.Link("https://example.org/article"),
            SharedImport.from("Un bel article https://example.org/article", "Un bel article"),
        )
        assertEquals(SharedImport.Link("https://example.org/a"), SharedImport.from("(https://example.org/a).", null))
    }

    @Test
    fun longTextWithALinkInsideIsText() {
        val text = "Bonjour ! ".repeat(30) + "https://example.org"
        assertEquals(SharedImport.Text(null, text), SharedImport.from(text, null))
    }

    @Test
    fun emptySharesAreIgnored() {
        assertEquals(null, SharedImport.from("  ", "x"))
        assertEquals(null, SharedImport.from(null, null))
    }
}
