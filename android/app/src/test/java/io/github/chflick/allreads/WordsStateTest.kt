package io.github.chflick.allreads

import io.github.chflick.allreads.data.WordSort
import io.github.chflick.allreads.ui.words.WordFilter
import io.github.chflick.allreads.ui.words.WordsState
import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.allreads_ffi.SavedTerm
import uniffi.allreads_ffi.WordStatus

class WordsStateTest {
    private fun term(
        id: Long,
        key: String,
        status: WordStatus,
        translation: String = "",
        importedFrom: String = "",
        waiting: Boolean = false,
    ) = SavedTerm(id, "fr", key, key, status, translation, "", null, importedFrom, waiting)

    private val terms = listOf(
        term(1, "chat", WordStatus.Learning(2u), "cat"),
        term(2, "chien", WordStatus.WellKnown, "dog"),
        term(3, "le", WordStatus.Ignored),
    )

    @Test
    fun countsKnownAndLearningWords() {
        val state = WordsState(terms = terms)
        assertEquals(1, state.knownCount)
        assertEquals(1, state.learningCount)
    }

    @Test
    fun filtersByStatus() {
        assertEquals(listOf(1L), WordsState(terms = terms, filter = WordFilter.Learning).visible.map { it.id })
        assertEquals(listOf(2L), WordsState(terms = terms, filter = WordFilter.Known).visible.map { it.id })
        assertEquals(listOf(3L), WordsState(terms = terms, filter = WordFilter.Ignored).visible.map { it.id })
        assertEquals(3, WordsState(terms = terms).visible.size)
    }

    @Test
    fun searchMatchesWordsAndTranslationsIgnoringCase() {
        assertEquals(listOf(1L), WordsState(terms = terms, query = " CHA ").visible.map { it.id })
        assertEquals(listOf(2L), WordsState(terms = terms, query = "Dog").visible.map { it.id })
        assertEquals(emptyList<Long>(), WordsState(terms = terms, query = "cat", filter = WordFilter.Known).visible.map { it.id })
    }

    @Test
    fun recentlyAddedComesFirstByDefaultAndTheOtherOrdersWork() {
        val words = listOf(
            term(1, "été", WordStatus.WellKnown),
            term(2, "zèbre", WordStatus.Learning(4u)),
            term(3, "abri", WordStatus.Learning(1u)),
            term(4, "eau", WordStatus.Ignored),
            term(5, "etre", WordStatus.Learning(1u)),
        )
        fun ids(sort: WordSort) = WordsState(terms = words, sort = sort).visible.map { it.id }
        assertEquals(listOf(5L, 4L, 3L, 2L, 1L), ids(WordSort.Recent))
        // "été" sorts with "ete", before "etre", not after "zèbre".
        assertEquals(listOf(3L, 4L, 1L, 5L, 2L), ids(WordSort.Alphabetical))
        // Least known first; the same level by A–Z.
        assertEquals(listOf(3L, 5L, 2L, 1L, 4L), ids(WordSort.Level))
    }

    @Test
    fun waitingAndImportedFiltersCombineWithTheLevels() {
        val words = listOf(
            term(1, "chat", WordStatus.Learning(2u), waiting = true),
            term(2, "chien", WordStatus.WellKnown, importedFrom = "Anki · Vocabulaire"),
            term(3, "oiseau", WordStatus.Learning(3u), importedFrom = "Anki · Vocabulaire", waiting = true),
        )
        val state = WordsState(terms = words)
        assertEquals(2, state.waitingCount)
        assertEquals(2, state.importedCount)
        assertEquals(listOf(3L, 1L), state.copy(waitingOnly = true).visible.map { it.id })
        assertEquals(listOf(3L, 2L), state.copy(importedOnly = true).visible.map { it.id })
        assertEquals(listOf(3L), state.copy(waitingOnly = true, importedOnly = true).visible.map { it.id })
        assertEquals(emptyList<Long>(), state.copy(importedOnly = true, filter = WordFilter.Ignored).visible.map { it.id })
        assertEquals(true, state.copy(waitingOnly = true).narrowed)
        assertEquals(false, state.narrowed)
    }
}
