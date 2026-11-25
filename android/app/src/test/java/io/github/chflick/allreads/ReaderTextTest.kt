package io.github.chflick.allreads

import io.github.chflick.allreads.reader.TokenText
import io.github.chflick.allreads.reader.dictionaryLink
import io.github.chflick.allreads.reader.firstRowOf
import io.github.chflick.allreads.reader.readerRows
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.allreads_ffi.ChapterBlock
import uniffi.allreads_ffi.Token
import uniffi.allreads_ffi.WordStatus

class ReaderTextTest {
    private fun word(text: String, sentence: UInt = 0u) =
        Token(text, text.lowercase(), WordStatus.New, sentence, false, false)

    private fun gap(text: String, sentence: UInt = 0u) = Token(text, null, null, sentence, false, false)

    // Block 0: heading "Titre". Block 1: "L'homme lit. Fin."
    private val blocks = listOf(
        ChapterBlock(headingLevel = 1u, tokens = listOf(word("Titre"))),
        ChapterBlock(
            headingLevel = null,
            tokens = listOf(
                word("L'"), word("homme"), gap(" "), word("lit"), gap("."),
                gap(" ", 1u), word("Fin", 1u), gap(".", 1u),
            ),
        ),
    )

    @Test
    fun eachSentenceBecomesARowWithoutItsLeadingSpace() {
        val rows = readerRows(blocks)
        assertEquals(listOf("Titre", "L'homme lit.", "Fin."), rows.map { it.content.text })
        assertEquals(listOf(true, true, false), rows.map { it.startsBlock })
        assertEquals(listOf(1, null, null), rows.map { it.headingLevel })
        assertEquals(listOf(0, 1, 1), rows.map { it.blockIndex })
        assertEquals(3, rows.map { it.key }.toSet().size)
    }

    @Test
    fun aSentenceStartingWithSpacedPunctuationLosesOnlyTheSpace() {
        val dialogue = ChapterBlock(
            headingLevel = null,
            tokens = listOf(word("Il"), gap("."), gap(" « ", 1u), word("Oui", 1u), gap(" »", 1u)),
        )
        val rows = readerRows(listOf(dialogue))
        assertEquals(listOf("Il.", "« Oui »"), rows.map { it.content.text })
        // Taps still map to the right word after the trim.
        assertEquals(1, rows[1].content.wordAt(3))
    }

    @Test
    fun aSavedBlockMapsToItsFirstRow() {
        val rows = readerRows(blocks)
        assertEquals(0, rows.firstRowOf(0))
        assertEquals(1, rows.firstRowOf(1))
        assertEquals(0, rows.firstRowOf(9))
    }

    @Test
    fun anOffsetMapsToItsToken() {
        val text = TokenText(blocks[1].tokens)
        assertEquals("L'homme lit. Fin.", text.text)
        assertEquals(0, text.tokenAt(0))
        assertEquals(1, text.tokenAt(6))
        assertEquals(2, text.tokenAt(7))
        assertNull(text.tokenAt(text.text.length))
        assertNull(text.tokenAt(-1))
    }

    @Test
    fun aTapJustAfterAWordStillPicksTheWord() {
        val text = TokenText(blocks[1].tokens)
        assertEquals(1, text.wordAt(7))
        assertEquals(3, text.wordAt(9))
    }

    @Test
    fun dictionaryLinksEncodeTheWord() {
        assertEquals(
            "http://www.wordreference.com/fren/peut-%C3%AAtre",
            dictionaryLink("http://www.wordreference.com/fren/{term}", "peut-être"),
        )
        assertEquals("https://x.org/?q=un%20deux", dictionaryLink("https://x.org/?q={term}", "un deux"))
        assertNull(dictionaryLink("", "mot"))
    }
}
