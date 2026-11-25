package io.github.chflick.allreads.reader

import uniffi.allreads_ffi.ChapterBlock
import uniffi.allreads_ffi.Token

/**
 * A block's text with the character offset where each token starts, so a tap
 * on the rendered text can be mapped back to its token. Plain Kotlin, unit-tested.
 */
class BlockText(val block: ChapterBlock) {
    val text: String = block.tokens.joinToString("") { it.text }

    /** `starts[i]` is the offset of token `i` in [text]. */
    val starts: IntArray = IntArray(block.tokens.size).also { starts ->
        var offset = 0
        block.tokens.forEachIndexed { i, token ->
            starts[i] = offset
            offset += token.text.length
        }
    }

    /** The index of the token that contains character `offset`, or null past the end. */
    fun tokenAt(offset: Int): Int? {
        if (offset < 0 || offset >= text.length) return null
        var low = 0
        var high = starts.size - 1
        while (low < high) {
            val mid = (low + high + 1) / 2
            if (starts[mid] <= offset) low = mid else high = mid - 1
        }
        return low
    }

    /**
     * The word token at a tapped offset. A tap on the right half of a word's
     * last letter reports the offset after it, so the token before is tried too.
     */
    fun wordAt(offset: Int): Int? =
        listOf(offset, offset - 1)
            .mapNotNull { tokenAt(it) }
            .firstOrNull { block.tokens[it].termKey != null }

    /** The sentence a token belongs to, trimmed, e.g. as context for a translation. */
    fun sentenceOf(tokenIndex: Int): String {
        val sentence = block.tokens[tokenIndex].sentence
        return block.tokens.filter { it.sentence == sentence }.joinToString("") { it.text }.trim()
    }
}

/** True if this token is a word. */
val Token.isWord: Boolean get() = termKey != null
