package io.github.chflick.allreads.reader

import java.net.URLEncoder
import uniffi.allreads_ffi.ChapterBlock
import uniffi.allreads_ffi.Token

/**
 * A run of tokens with the character offset where each one starts, so a tap
 * on the rendered text maps back to its token. Plain Kotlin, unit-tested.
 */
class TokenText(val tokens: List<Token>) {
    val text: String = tokens.joinToString("") { it.text }

    /** `starts[i]` is the offset of token `i` in [text]. */
    private val starts: IntArray = IntArray(tokens.size).also { starts ->
        var offset = 0
        tokens.forEachIndexed { i, token ->
            starts[i] = offset
            offset += token.text.length
        }
    }

    /** The index of the token that contains character `offset`, or null outside the text. */
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
            .firstOrNull { tokens[it].termKey != null }
}

/** One row of the reader: a sentence, or a heading. */
data class ReaderRow(
    val blockIndex: Int,
    val headingLevel: Int?,
    /** True for the first sentence of a block, which starts a new paragraph. */
    val startsBlock: Boolean,
    val content: TokenText,
) {
    /** The sentence's text without the space before it, e.g. for translation. */
    val sentence: String get() = content.text.trim()

    /** A stable key for this row within a chapter. */
    val key: String get() = "$blockIndex:${content.tokens.firstOrNull()?.sentence ?: 0u}"
}

/**
 * Splits a chapter into rows of one sentence each. A sentence's leading
 * space is dropped, since each sentence starts its own line.
 */
fun readerRows(blocks: List<ChapterBlock>): List<ReaderRow> =
    blocks.flatMapIndexed { blockIndex, block ->
        block.tokens
            .groupBy { it.sentence }
            .values
            .mapIndexedNotNull { i, sentence ->
                val trimmed = sentence.dropWhile { it.termKey == null && it.text.isBlank() }
                if (trimmed.isEmpty()) {
                    null
                } else {
                    ReaderRow(
                        blockIndex = blockIndex,
                        headingLevel = block.headingLevel?.toInt(),
                        startsBlock = i == 0,
                        content = TokenText(trimmed),
                    )
                }
            }
    }

/** The index of the first row in `block` or later, for resuming at a saved block. */
fun List<ReaderRow>.firstRowOf(block: Int): Int = indexOfFirst { it.blockIndex >= block }.coerceAtLeast(0)

/** A web dictionary link: `{term}` in the template replaced by the URL-encoded word. */
fun dictionaryLink(template: String, word: String): String? =
    template.takeIf { it.contains("{term}") }
        ?.replace("{term}", URLEncoder.encode(word, "UTF-8").replace("+", "%20"))

/** True if this token is a word. */
val Token.isWord: Boolean get() = termKey != null
