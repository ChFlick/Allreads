package io.github.chflick.allreads

import io.github.chflick.allreads.data.Repository
import io.github.chflick.allreads.ui.vocab.VocabQuiz
import java.io.File
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import uniffi.allreads_ffi.WordStatus

/** A whole vocabulary check from Kotlin, with the real core and a test dictionary of made-up words. */
class VocabQuizTest {
    @get:Rule
    val temp = TemporaryFolder()

    private val fixtures = File(System.getProperty("allreads.fixtures") ?: error("allreads.fixtures not set"))

    @Test
    fun aCheckMarksTheCommonWordsAReaderKnowsAndCanBeUndone() = runBlocking {
        val data = temp.newFolder("data")
        val repository = Repository(data)
        assertFalse(repository.vocabularyCheckAvailable("fr"))
        File(fixtures, "frequencies.sqlite").copyTo(File(data, "dictionaries/fr-en.sqlite"))
        assertTrue(repository.vocabularyCheckAvailable("fr"))

        // A reader who knows the 1,500 most common words and says no to made-up ones.
        val quiz = VocabQuiz(repository.startVocabularyCheck("fr"))
        while (true) {
            val word = quiz.current() ?: break
            quiz.answer(word.rank?.let { it < 1_500u } ?: false)
        }
        assertTrue("asked ${quiz.answered}", quiz.answered in 30..100)
        val result = quiz.result()
        assertTrue(result.reliable)
        val safe = result.choices.first()
        assertTrue("$safe", safe.words > 300u && safe.expectedAccuracy > 0.9)

        // The marked words, plus the extra form of the most common one.
        val session = repository.startVocabularyCheck("fr")
        val again = VocabQuiz(session)
        while (true) {
            val word = again.current() ?: break
            again.answer(word.rank?.let { it < 1_500u } ?: false)
        }
        val applied = repository.applyVocabularyCheck(session, 0)
        val added = applied.formsAdded.toInt()
        val words = repository.listTerms("fr")
        assertEquals(added, words.size)
        assertTrue(words.all { it.status == WordStatus.WellKnown && it.importedFrom == "Vocabulary check" })
        assertTrue(words.any { it.termKey == "extraform" })
        // The option's count is dictionary forms; the extra form comes on top.
        assertEquals(applied.words.toInt() + 1, added)

        assertEquals(added, repository.vocabularyCheckCount("fr"))
        assertEquals(added, repository.undoVocabularyCheck("fr"))
        assertTrue(repository.listTerms("fr").isEmpty())
    }

    @Test
    fun backAsksThePreviousWordAgainWithinABand() = runBlocking {
        val data = temp.newFolder("data")
        File(fixtures, "frequencies.sqlite").copyTo(File(data, "dictionaries/fr-en.sqlite"))
        val quiz = VocabQuiz(Repository(data).startVocabularyCheck("fr"))
        assertFalse(quiz.canGoBack)
        val first = quiz.current()!!
        quiz.answer(true)
        val second = quiz.current()!!
        assertTrue(quiz.canGoBack)
        quiz.goBack()
        assertEquals(first, quiz.current())
        assertEquals(0, quiz.answered)
        quiz.answer(false)
        assertEquals(second, quiz.current())
    }
}
