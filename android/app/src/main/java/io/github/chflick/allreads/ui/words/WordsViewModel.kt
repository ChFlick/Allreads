package io.github.chflick.allreads.ui.words

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import io.github.chflick.allreads.data.Repository
import io.github.chflick.allreads.ui.describe
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.allreads_ffi.SavedTerm
import uniffi.allreads_ffi.TermInput
import uniffi.allreads_ffi.WordStatus

/** Which saved words the list shows. */
enum class WordFilter(val label: String) { All("All"), Learning("Learning"), Known("Known"), Ignored("Ignored") }

data class WordsState(
    val languages: List<String> = emptyList(),
    val language: String? = null,
    val terms: List<SavedTerm> = emptyList(),
    val query: String = "",
    val filter: WordFilter = WordFilter.All,
    val editing: SavedTerm? = null,
    val message: String? = null,
) {
    val learningCount: Int get() = terms.count { it.status is WordStatus.Learning }
    val knownCount: Int get() = terms.count { it.status == WordStatus.WellKnown }

    /** The terms matching the filter and search, by key or translation. */
    val visible: List<SavedTerm>
        get() = terms.filter { term ->
            val statusMatches = when (filter) {
                WordFilter.All -> true
                WordFilter.Learning -> term.status is WordStatus.Learning
                WordFilter.Known -> term.status == WordStatus.WellKnown
                WordFilter.Ignored -> term.status == WordStatus.Ignored
            }
            val q = query.trim().lowercase()
            statusMatches && (q.isEmpty() || term.termKey.contains(q) || term.translation.lowercase().contains(q))
        }
}

/** PRD F17: the word list, with search and a filter by status and language. */
class WordsViewModel(private val repository: Repository) : ViewModel() {
    private val _state = MutableStateFlow(WordsState())
    val state: StateFlow<WordsState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            val languages = runCatching { repository.languageCodes() }.getOrDefault(emptyList())
            // Start with the language of the most recently added book, if any.
            val recent = runCatching { repository.library().lastOrNull()?.languageCode }.getOrNull()
            _state.update { it.copy(languages = languages, language = recent ?: languages.firstOrNull()) }
            refresh()
        }
    }

    fun refresh() {
        val language = _state.value.language ?: return
        viewModelScope.launch {
            runCatching { repository.listTerms(language) }
                .onSuccess { terms -> _state.update { it.copy(terms = terms) } }
                .onFailure { e -> _state.update { it.copy(message = describe(e)) } }
        }
    }

    fun chooseLanguage(code: String) {
        _state.update { it.copy(language = code, terms = emptyList()) }
        refresh()
    }

    fun search(query: String) = _state.update { it.copy(query = query) }

    fun filter(filter: WordFilter) = _state.update { it.copy(filter = filter) }

    fun edit(term: SavedTerm?) = _state.update { it.copy(editing = term) }

    /** Saves the edited word, or forgets it for New. */
    fun save(term: SavedTerm, status: WordStatus, translation: String, notes: String) {
        viewModelScope.launch {
            val result = runCatching {
                if (status == WordStatus.New) {
                    repository.forgetTerm(term.languageCode, term.displayText)
                } else {
                    repository.saveTerm(
                        TermInput(
                            languageCode = term.languageCode,
                            text = term.displayText,
                            status = status,
                            translation = translation.trim(),
                            notes = notes.trim(),
                            lemma = term.lemma,
                        ),
                    )
                }
            }
            _state.update { it.copy(editing = null, message = result.exceptionOrNull()?.let(::describe)) }
            refresh()
        }
    }

    fun messageShown() = _state.update { it.copy(message = null) }
}
