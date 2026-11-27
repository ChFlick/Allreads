package io.github.chflick.allreads.ui.vocab

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.allreads_ffi.VocabCheckSession
import uniffi.allreads_ffi.VocabResult
import uniffi.allreads_ffi.VocabWord

/**
 * Asks a vocabulary check one word at a time. The core hands out a band of
 * words at once; this keeps them in order and passes each answer back. Plain
 * Kotlin, so JVM tests can run a whole check against the real core.
 */
class VocabQuiz(private val session: VocabCheckSession) {
    private val waiting = ArrayDeque<VocabWord>()

    /** Words answered so far. */
    var answered: Int = 0
        private set

    /** The word to ask now, or null when the check is over. */
    suspend fun current(): VocabWord? {
        if (waiting.isEmpty()) waiting.addAll(withContext(Dispatchers.IO) { session.nextWords() })
        return waiting.firstOrNull()
    }

    /** Answers the current word. */
    suspend fun answer(yes: Boolean) {
        val word = waiting.removeFirstOrNull() ?: return
        withContext(Dispatchers.IO) { session.answer(word, yes) }
        answered++
    }

    suspend fun result(): VocabResult = withContext(Dispatchers.IO) { session.result() }

    companion object {
        /** About how many words a check asks, for the progress bar. */
        const val TYPICAL_LENGTH = 65
    }
}
