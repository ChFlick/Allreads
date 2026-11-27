package io.github.chflick.allreads.ui.vocab

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import io.github.chflick.allreads.data.Repository
import io.github.chflick.allreads.data.SettingsStore
import io.github.chflick.allreads.ui.describe
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import uniffi.allreads_ffi.VocabCheckSession
import uniffi.allreads_ffi.VocabResult

/** Where a vocabulary check is. */
sealed interface VocabCheckState {
    data object Starting : VocabCheckState

    /** A word to answer, and how far along the check is (0 to 1). */
    data class Asking(val word: String, val progress: Float) : VocabCheckState

    /** The result; `chosen` is the index of the selected choice. */
    data class Result(val result: VocabResult, val chosen: Int = 0, val applying: Boolean = false) : VocabCheckState

    /** Words were marked known: `words` as the chosen option counted them, `forms` saved with their other forms. */
    data class Done(val words: Int, val forms: Int) : VocabCheckState

    data class Failed(val message: String) : VocabCheckState
}

/**
 * The vocabulary check, from the first word to marking the result. One
 * instance serves every check: [open] starts a fresh one per opening.
 */
class VocabCheckViewModel(
    private val repository: Repository,
    /** Remembers which languages were checked, so the library stops offering it; null in tests. */
    private val settings: SettingsStore? = null,
) : ViewModel() {
    private val _state = MutableStateFlow<VocabCheckState>(VocabCheckState.Starting)
    val state: StateFlow<VocabCheckState> = _state.asStateFlow()

    /** The language being checked. */
    var languageCode: String = ""
        private set

    private var opening = -1
    private var session: VocabCheckSession? = null
    private var quiz: VocabQuiz? = null
    /** The running start or answer; one at a time, so a quick double tap cannot answer a word not shown yet. */
    private var job: Job? = null

    /**
     * Starts a check in `language` for one opening of the screen; repeat calls
     * for the same opening (e.g. on rotation) keep it. Returns true when a new
     * check started.
     */
    fun open(language: String, opening: Int): Boolean {
        if (opening == this.opening) return false
        this.opening = opening
        languageCode = language
        start()
        return true
    }

    /** Starts (or restarts) the check with fresh words. */
    fun start() {
        job?.cancel()
        _state.value = VocabCheckState.Starting
        session = null
        quiz = null
        val language = languageCode
        job = viewModelScope.launch {
            runCatching {
                val started = repository.startVocabularyCheck(language)
                session = started
                quiz = VocabQuiz(started)
                askNext()
            }.onFailure { fail(it) }
        }
    }

    fun answer(yes: Boolean) {
        val quiz = quiz ?: return
        if (_state.value !is VocabCheckState.Asking) return
        if (job?.isActive == true) return
        job = viewModelScope.launch {
            runCatching {
                quiz.answer(yes)
                askNext()
            }.onFailure { fail(it) }
        }
    }

    /** Shows a failure, unless the work was only cancelled by a restart. */
    private fun fail(error: Throwable) {
        if (error is CancellationException) return
        _state.value = VocabCheckState.Failed(describe(error))
    }

    private suspend fun askNext() {
        val quiz = quiz ?: return
        val word = quiz.current()
        _state.value = if (word != null) {
            val progress = (quiz.answered.toFloat() / VocabQuiz.TYPICAL_LENGTH).coerceAtMost(0.95f)
            VocabCheckState.Asking(word.word, progress)
        } else {
            // A finished check counts as done, whatever the reader marks.
            val language = languageCode
            settings?.update { it.copy(vocabChecked = it.vocabChecked + language) }
            VocabCheckState.Result(quiz.result())
        }
    }

    fun choose(index: Int) {
        val current = _state.value as? VocabCheckState.Result ?: return
        _state.value = current.copy(chosen = index)
    }

    /** Marks the chosen words known. */
    fun apply() {
        val current = _state.value as? VocabCheckState.Result ?: return
        val session = session ?: return
        if (current.applying) return
        _state.value = current.copy(applying = true)
        viewModelScope.launch {
            runCatching { repository.applyVocabularyCheck(session, current.chosen) }
                .onSuccess { _state.value = VocabCheckState.Done(it.words.toInt(), it.formsAdded.toInt()) }
                .onFailure { _state.value = VocabCheckState.Failed(describe(it)) }
        }
    }
}
