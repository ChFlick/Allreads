package io.github.chflick.allreads.ui.words

import java.text.Collator
import io.github.chflick.allreads.data.WordSort
import io.github.chflick.allreads.data.SettingsStore
import androidx.annotation.StringRes
import io.github.chflick.allreads.R
import io.github.chflick.allreads.ui.AppText

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
import io.github.chflick.allreads.ui.theme.longLabel
import uniffi.allreads_ffi.WordProgress
import uniffi.allreads_ffi.TermInput
import uniffi.allreads_ffi.WordStatus

/** Which saved words the list shows. */
enum class WordFilter(@StringRes private val labelId: Int) {
    All(R.string.all), Learning(R.string.learning), Known(R.string.known_2), Ignored(R.string.ignored);

    val label: String get() = AppText.get(labelId)
}

data class WordsState(
    val languages: List<String> = emptyList(),
    val language: String? = null,
    val terms: List<SavedTerm> = emptyList(),
    val query: String = "",
    val filter: WordFilter = WordFilter.All,
    val sort: WordSort = WordSort.Recent,
    /** Only words still waiting to go to AnkiDroid. */
    val waitingOnly: Boolean = false,
    /** Only words imported from Anki or a file. */
    val importedOnly: Boolean = false,
    val editing: SavedTerm? = null,
    val message: String? = null,
    /** Known and learning words per day in this language, for the chart. */
    val progress: WordProgress = WordProgress(emptyList(), 0u),
    /** The last swipe, offered for undo. */
    val undo: WordUndo? = null,
) {
    /** Words per filter chip, counted once per state, not on every recomposition. */
    private val counts: Map<WordFilter, Int> by lazy {
        terms.groupingBy { term ->
            when (term.status) {
                is WordStatus.Learning -> WordFilter.Learning
                WordStatus.WellKnown -> WordFilter.Known
                WordStatus.Ignored -> WordFilter.Ignored
                WordStatus.New -> WordFilter.All
            }
        }.eachCount() + (WordFilter.All to terms.size)
    }

    val learningCount: Int get() = count(WordFilter.Learning)
    val knownCount: Int get() = count(WordFilter.Known)

    /** How many words a filter chip would show, for its label. */
    fun count(filter: WordFilter): Int = counts[filter] ?: 0

    /** Words still waiting to go to AnkiDroid. */
    val waitingCount: Int by lazy { terms.count { it.waitingForAnki } }

    /** Words imported from Anki or a file. */
    val importedCount: Int by lazy { terms.count { it.importedFrom.isNotBlank() } }

    /** True when something narrows the list beyond the language. */
    val narrowed: Boolean get() = query.isNotBlank() || filter != WordFilter.All || waitingOnly || importedOnly

    /** The terms matching the filters and search (by key or translation), in the chosen order. */
    val visible: List<SavedTerm> by lazy {
        val q = query.trim().lowercase()
        val shown = terms.filter { term ->
            val statusMatches = when (filter) {
                WordFilter.All -> true
                WordFilter.Learning -> term.status is WordStatus.Learning
                WordFilter.Known -> term.status == WordStatus.WellKnown
                WordFilter.Ignored -> term.status == WordStatus.Ignored
            }
            statusMatches &&
                (!waitingOnly || term.waitingForAnki) &&
                (!importedOnly || term.importedFrom.isNotBlank()) &&
                (q.isEmpty() || term.termKey.contains(q) || term.translation.lowercase().contains(q))
        }
        sortWords(shown, sort)
    }
}

/**
 * Orders words for the list. Ids grow as words are saved, so the highest
 * comes first for "Recently added". Alphabetical order follows the phone's
 * language rules, so "été" sorts with "ete", not after "z".
 */
fun sortWords(words: List<SavedTerm>, sort: WordSort): List<SavedTerm> {
    val collator = Collator.getInstance().apply { strength = Collator.PRIMARY }
    val alphabetical = Comparator<SavedTerm> { a, b -> collator.compare(a.termKey, b.termKey) }
    return when (sort) {
        WordSort.Recent -> words.sortedByDescending { it.id }
        WordSort.Alphabetical -> words.sortedWith(alphabetical)
        WordSort.Level -> words.sortedWith(compareBy<SavedTerm> { levelRank(it.status) }.then(alphabetical))
    }
}

/** Least known first: levels 1 to 5, then known, then ignored. */
private fun levelRank(status: WordStatus): Int = when (status) {
    is WordStatus.Learning -> status.level.toInt()
    WordStatus.WellKnown -> 6
    WordStatus.Ignored -> 7
    WordStatus.New -> 0
}

/** PRD F17: the word list, with search and a filter by status and language. */
class WordsViewModel(
    private val repository: Repository,
    /** Pushes saved changes to AnkiDroid. */
    private val syncAnki: () -> Unit = {},
    /** Remembers the chosen order; null in previews. */
    private val settings: SettingsStore? = null,
) : ViewModel() {
    private val _state = MutableStateFlow(WordsState(sort = settings?.settings?.value?.wordSort ?: WordSort.Recent))
    val state: StateFlow<WordsState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            // The languages of the books and of saved words (not every preset the
            // app knows), the most recently read first; all before anything is imported.
            val used = runCatching { repository.wordLanguageCodes() }.getOrDefault(emptyList())
            val languages = used.sorted().ifEmpty { runCatching { repository.languageCodes() }.getOrDefault(emptyList()) }
            val recent = used.firstOrNull()
            _state.update { it.copy(languages = languages, language = recent ?: languages.firstOrNull()) }
            refresh()
        }
    }

    fun refresh() {
        val language = _state.value.language ?: return
        viewModelScope.launch {
            // Without an AnkiDroid deck chosen every word waits, which says
            // nothing; the Waiting filter and badge are only for Anki users.
            val ankiInUse = runCatching { repository.ankiDeckId() != null }.getOrDefault(false)
            runCatching { repository.listTerms(language) }
                .map { terms -> if (ankiInUse) terms else terms.map { it.copy(waitingForAnki = false) } }
                .onSuccess { terms -> _state.update { it.copy(terms = terms, waitingOnly = it.waitingOnly && ankiInUse) } }
                .onFailure { e -> _state.update { it.copy(message = describe(e)) } }
            runCatching { repository.wordProgress(language) }.onSuccess { progress ->
                _state.update { it.copy(progress = progress) }
            }
        }
    }

    fun chooseLanguage(code: String) {
        _state.update { it.copy(language = code, terms = emptyList()) }
        refresh()
    }

    fun search(query: String) = _state.update { it.copy(query = query) }

    fun filter(filter: WordFilter) = _state.update { it.copy(filter = filter) }

    fun sort(sort: WordSort) {
        _state.update { it.copy(sort = sort) }
        settings?.update { it.copy(wordSort = sort) }
    }

    /** Switches the "Waiting for AnkiDroid" filter on or off. */
    fun toggleWaiting() = _state.update { it.copy(waitingOnly = !it.waitingOnly) }

    /** Switches the "Imported" filter on or off. */
    fun toggleImported() = _state.update { it.copy(importedOnly = !it.importedOnly) }

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
                            sentence = null,
                            sentenceTranslation = null,
                            sourceBook = null,
                            sourceChapter = null,
                        ),
                    )
                }
            }
            _state.update { it.copy(editing = null, message = result.exceptionOrNull()?.let(::describe)) }
            refresh()
            if (result.isSuccess) syncAnki()
        }
    }

    fun messageShown() = _state.update { it.copy(message = null) }

    /** A swipe to the left: one level up, 5 becomes known, as the reader's double tap. */
    fun raise(term: SavedTerm) {
        val next = when (val status = term.status) {
            is WordStatus.Learning -> if (status.level < 5u) WordStatus.Learning((status.level + 1u).toUByte()) else WordStatus.WellKnown
            else -> return
        }
        viewModelScope.launch {
            val result = runCatching { repository.setTermStatus(term.languageCode, term.termKey, next) }
            _state.update {
                it.copy(
                    undo = result.getOrNull()?.let { before -> WordUndo(AppText.get(R.string.word_level_change, term.displayText, next.longLabel().lowercase()), term.copy(status = before), forgotten = false) },
                    message = result.exceptionOrNull()?.let(::describe),
                )
            }
            refresh()
            syncAnki()
        }
    }

    /** A swipe to the right: the word is forgotten (its Anki card is suspended after the Undo window), with Undo. */
    fun forget(term: SavedTerm) {
        viewModelScope.launch {
            val result = runCatching { repository.forgetTerm(term.languageCode, term.termKey) }
            _state.update {
                it.copy(
                    undo = WordUndo(AppText.get(R.string.word_forgotten, term.displayText), term, forgotten = true).takeIf { result.isSuccess },
                    message = result.exceptionOrNull()?.let(::describe),
                )
            }
            refresh()
        }
    }

    fun undoShown() = _state.update { it.copy(undo = null) }

    /** Puts a swiped word back as it was: its level, or the whole word after a forget. */
    fun undo(undo: WordUndo) {
        val term = undo.term
        viewModelScope.launch {
            runCatching {
                if (undo.forgotten) {
                    // The core kept the whole word: sentence, import badge and card.
                    repository.undoForget(term.languageCode, term.termKey)
                } else {
                    repository.setTermStatus(term.languageCode, term.termKey, term.status)
                }
            }.onFailure { e -> _state.update { it.copy(message = describe(e)) } }
            refresh()
            syncAnki()
        }
    }
}

/** What a swipe did, for the Undo snackbar: the word as it was before. */
data class WordUndo(val message: String, val term: SavedTerm, val forgotten: Boolean)
