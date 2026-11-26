package io.github.chflick.allreads

import io.github.chflick.allreads.ui.words.WordFilter
import io.github.chflick.allreads.ui.words.WordsState
import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.allreads_ffi.SavedTerm
import uniffi.allreads_ffi.WordStatus

class WordsStateTest {
    private fun term(id: Long, key: String, status: WordStatus, translation: String = "") =
        SavedTerm(id, "fr", key, key, status, translation, "", null, "")

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
}
