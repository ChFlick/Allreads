package io.github.chflick.allreads

import io.github.chflick.allreads.reader.TokenText
import io.github.chflick.allreads.reader.dictionaryLink
import io.github.chflick.allreads.reader.firstRowOf
import io.github.chflick.allreads.reader.readerRows
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.allreads_ffi.ChapterBlock
import uniffi.allreads_ffi.PhraseMark
import uniffi.allreads_ffi.Token
import uniffi.allreads_ffi.WordStatus

class ReaderTextTest {
    private fun word(text: String, sentence: UInt = 0u) =
        Token(text, text.lowercase(), WordStatus.New, sentence, false, false, null, false, false)

    private fun gap(text: String, sentence: UInt = 0u) = Token(text, null, null, sentence, false, false, null, false, false)

    private fun Token.inPhrase(id: UInt) =
        copy(phrase = PhraseMark(id, "pierre précieuse", WordStatus.Learning(2u)))

    @Test
    fun aPhraseIsFoundAroundAnyOfItsTokens() {
        // "une Pierre précieuse ici", with "Pierre précieuse" saved.
        val text = TokenText(
            listOf(
                word("une"), gap(" "), word("Pierre").inPhrase(0u), gap(" ").inPhrase(0u),
                word("précieuse").inPhrase(0u), gap(" "), word("ici"),
            ),
        )
        assertEquals(2..4, text.phraseRange(3))
        assertEquals(2..4, text.phraseRange(2))
        assertNull(text.phraseRange(0))
        assertEquals("Pierre précieuse", text.textOf(2..4))
        assertEquals(4 until 20, text.charRange(2..4))
        assertEquals(listOf(0, 2, 4, 6), text.wordIndexes)
    }

    @Test
    fun twoOccurrencesSideBySideStayApart() {
        val text = TokenText(listOf(word("a").inPhrase(0u), word("b").inPhrase(1u)))
        assertEquals(0..0, text.phraseRange(0))
        assertEquals(1..1, text.phraseRange(1))
    }

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
    fun rangesCoverTheMatchingTokens() {
        val text = TokenText(blocks[1].tokens)
        // "L'homme lit. Fin." – the words, by character range.
        assertEquals(listOf(0..1, 2..6, 8..10, 13..15), text.ranges { it.termKey != null })
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
