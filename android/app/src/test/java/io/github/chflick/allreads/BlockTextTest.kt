package io.github.chflick.allreads

import io.github.chflick.allreads.reader.BlockText
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.allreads_ffi.ChapterBlock
import uniffi.allreads_ffi.Token
import uniffi.allreads_ffi.WordStatus

class BlockTextTest {
    private fun word(text: String, sentence: UInt = 0u) =
        Token(text, text.lowercase(), WordStatus.New, sentence, false, false)

    private fun gap(text: String, sentence: UInt = 0u) = Token(text, null, null, sentence, false, false)

    // "L'homme lit. Fin."
    private val block = BlockText(
        ChapterBlock(
            headingLevel = null,
            tokens = listOf(
                word("L'"), word("homme"), gap(" "), word("lit"), gap("."),
                gap(" ", 1u), word("Fin", 1u), gap(".", 1u),
            ),
        ),
    )

    @Test
    fun textJoinsTheTokens() {
        assertEquals("L'homme lit. Fin.", block.text)
    }

    @Test
    fun anOffsetMapsToItsToken() {
        assertEquals(0, block.tokenAt(0))
        assertEquals(1, block.tokenAt(2))
        assertEquals(1, block.tokenAt(6))
        assertEquals(2, block.tokenAt(7))
        assertNull(block.tokenAt(block.text.length))
        assertNull(block.tokenAt(-1))
    }

    @Test
    fun aTapJustAfterAWordStillPicksTheWord() {
        // Offset 7 is the space after "homme": the word before it wins.
        assertEquals(1, block.wordAt(7))
        assertEquals(3, block.wordAt(9))
    }

    @Test
    fun theSentenceOfAWordIsItsTrimmedText() {
        assertEquals("L'homme lit.", block.sentenceOf(1))
        assertEquals("Fin.", block.sentenceOf(6))
    }
}
