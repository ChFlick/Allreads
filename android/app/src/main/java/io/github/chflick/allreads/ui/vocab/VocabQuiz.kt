package io.github.chflick.allreads.ui.vocab

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.allreads_ffi.VocabAnswer
import uniffi.allreads_ffi.VocabCheckSession
import uniffi.allreads_ffi.VocabResult
import uniffi.allreads_ffi.VocabWord

/**
 * Asks a vocabulary check one word at a time. The core hands out a band of
 * words at once; this keeps them in order and sends the band's answers back
 * in one call before asking for the next band. Plain Kotlin, so JVM tests
 * can run a whole check against the real core.
 */
class VocabQuiz(private val session: VocabCheckSession) {
    private val waiting = ArrayDeque<VocabWord>()
    private val answers = mutableListOf<VocabAnswer>()

    /** Words answered so far. */
    var answered: Int = 0
        private set

    /** The word to ask now, or null when the check is over. */
    suspend fun current(): VocabWord? {
        if (waiting.isEmpty()) {
            send()
            waiting.addAll(withContext(Dispatchers.IO) { session.nextWords() })
        }
        return waiting.firstOrNull()
    }

    /** Answers the current word; the band's answers go to the core together. */
    fun answer(yes: Boolean) {
        val word = waiting.removeFirstOrNull() ?: return
        answers += VocabAnswer(word, yes)
        answered++
    }

    suspend fun result(): VocabResult {
        send()
        return withContext(Dispatchers.IO) { session.result() }
    }

    /** Sends the answers given since the last call. */
    private suspend fun send() {
        if (answers.isEmpty()) return
        val batch = answers.toList()
        answers.clear()
        withContext(Dispatchers.IO) { session.answerAll(batch) }
    }

    companion object {
        /** About how many words a check asks, for the progress bar. */
        const val TYPICAL_LENGTH = 65
    }
}
