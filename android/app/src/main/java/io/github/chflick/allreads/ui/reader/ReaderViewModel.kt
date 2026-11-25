package io.github.chflick.allreads.ui.reader

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import io.github.chflick.allreads.data.Services
import io.github.chflick.allreads.reader.ReaderRow
import io.github.chflick.allreads.reader.dictionaryLink
import io.github.chflick.allreads.reader.readerRows
import io.github.chflick.allreads.ui.NO_DEEPL_KEY
import io.github.chflick.allreads.ui.describe
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.allreads_ffi.BookSummary
import uniffi.allreads_ffi.SavedTerm
import uniffi.allreads_ffi.TermInput
import uniffi.allreads_ffi.WordStatus

/** A sentence's translation, as shown under it. */
sealed interface SentenceTranslation {
    data object Loading : SentenceTranslation
    data class Shown(val text: String) : SentenceTranslation
    data class Failed(val message: String) : SentenceTranslation
}

/** The word in the drawer, with what the drawer shows. */
data class SelectedWord(
    val text: String,
    val sentence: String,
    /** The [ReaderRow.key] of the sentence, to find its shown translation. */
    val rowKey: String,
    val status: WordStatus,
    val saved: SavedTerm?,
    /** DeepL's in-context translation, when it has arrived. */
    val machineTranslation: String? = null,
    val translating: Boolean = false,
    val translationError: String? = null,
    val dictionaryUrl: String? = null,
)

data class ReaderState(
    val book: BookSummary? = null,
    val chapterIndex: UInt = 0u,
    val chapterTitle: String? = null,
    val rows: List<ReaderRow> = emptyList(),
    /** The block to scroll to when a chapter opens (the saved position). */
    val startBlock: Int = 0,
    /** Translations shown under sentences, by [ReaderRow.key]. */
    val translations: Map<String, SentenceTranslation> = emptyMap(),
    val selected: SelectedWord? = null,
    val message: String? = null,
)

/**
 * The reader's state. One instance serves every book: [open] starts a fresh
 * session for each book opening, while a rotation keeps the current one.
 */
class ReaderViewModel(private val services: Services) : ViewModel() {
    private val repository = services.repository
    private val _state = MutableStateFlow(ReaderState())
    val state: StateFlow<ReaderState> = _state.asStateFlow()

    private var bookId: Long = -1
    private var openId: Int = -1
    private var titles: List<String?> = emptyList()
    private var dictionaryTemplate: String = ""
    private var openJob: Job? = null
    private var selectJob: Job? = null

    /**
     * Opens a book at its saved position. `openId` identifies one opening of
     * the reader: calling again with the same id (after a rotation) does nothing.
     */
    fun open(bookId: Long, openId: Int) {
        if (this.bookId == bookId && this.openId == openId) return
        this.bookId = bookId
        this.openId = openId
        openJob?.cancel()
        selectJob?.cancel()
        titles = emptyList()
        dictionaryTemplate = ""
        _state.value = ReaderState()
        openJob = viewModelScope.launch {
            val book = repository.book(bookId)
            if (book == null) {
                _state.update { it.copy(message = "This book is no longer in the library") }
                return@launch
            }
            titles = runCatching { repository.chapterTitles(bookId) }.getOrDefault(emptyList())
            dictionaryTemplate = runCatching { repository.languageInfo(book.languageCode) }
                .getOrNull()?.dictionaryUrl.orEmpty()
            _state.update { it.copy(book = book) }
            load(book.chapterIndex, startBlock = book.paragraphIndex.toInt())
        }
    }

    private suspend fun load(index: UInt, startBlock: Int) {
        runCatching { repository.openChapter(bookId, index) }
            .onSuccess { chapter ->
                _state.update {
                    it.copy(
                        chapterIndex = index,
                        chapterTitle = titles.getOrNull(index.toInt()),
                        rows = readerRows(chapter.blocks),
                        startBlock = startBlock,
                        translations = emptyMap(),
                    )
                }
            }
            .onFailure { e -> _state.update { it.copy(message = describe(e)) } }
    }

    fun nextChapter() = goToChapter(_state.value.chapterIndex + 1u)

    fun previousChapter() {
        val index = _state.value.chapterIndex
        if (index > 0u) goToChapter(index - 1u)
    }

    private fun goToChapter(index: UInt) {
        val count = _state.value.book?.chapterCount ?: return
        if (index >= count) return
        viewModelScope.launch {
            load(index, startBlock = 0)
            savePosition(0)
        }
    }

    /** Remembers the first visible block, so reopening the book resumes there. */
    fun savePosition(block: Int) {
        val chapter = _state.value.chapterIndex
        viewModelScope.launch { runCatching { repository.savePosition(bookId, chapter, block.toUInt()) } }
    }

    /** Shows the sentence's translation under it, or hides it if shown. */
    fun toggleTranslation(row: ReaderRow) {
        val key = row.key
        if (_state.value.translations.containsKey(key)) {
            _state.update { it.copy(translations = it.translations - key) }
            return
        }
        _state.update { it.copy(translations = it.translations + (key to SentenceTranslation.Loading)) }
        viewModelScope.launch {
            val result = translate(row.sentence, context = null)
            val shown = result.fold(
                { SentenceTranslation.Shown(it) },
                { SentenceTranslation.Failed(it.message ?: describe(it)) },
            )
            _state.update { state ->
                if (state.translations.containsKey(key)) state.copy(translations = state.translations + (key to shown))
                else state
            }
        }
    }

    fun select(row: ReaderRow, tokenIndex: Int) {
        val token = row.content.tokens.getOrNull(tokenIndex) ?: return
        val language = _state.value.book?.languageCode ?: return
        // A newer tap wins: an older lookup that finishes later must not replace it.
        selectJob?.cancel()
        selectJob = viewModelScope.launch {
            val saved = runCatching { repository.findTerm(language, token.text) }.getOrNull()
            val selected = SelectedWord(
                text = token.text,
                sentence = row.sentence,
                rowKey = row.key,
                status = token.status ?: WordStatus.New,
                saved = saved,
                dictionaryUrl = dictionaryLink(dictionaryTemplate, token.text),
            )
            _state.update { it.copy(selected = selected) }
            // Unsaved (or untranslated) words get DeepL's translation in context.
            if (saved?.translation.isNullOrBlank()) translateSelected()
        }
    }

    /** Fetches DeepL's translation of the selected word, with its sentence as context. */
    fun translateSelected() {
        val word = _state.value.selected ?: return
        _state.update { it.copy(selected = word.copy(translating = true, translationError = null)) }
        viewModelScope.launch {
            val result = translate(word.text, context = word.sentence)
            _state.update { state ->
                val current = state.selected?.takeIf { it.text == word.text } ?: return@update state
                state.copy(
                    selected = current.copy(
                        translating = false,
                        machineTranslation = result.getOrNull(),
                        translationError = result.exceptionOrNull()?.let { it.message ?: describe(it) },
                    ),
                )
            }
        }
    }

    private suspend fun translate(text: String, context: String?): Result<String> {
        val book = _state.value.book ?: return Result.failure(IllegalStateException("No book"))
        // Decrypting goes through the Android Keystore, so keep it off the main thread.
        val key = withContext(Dispatchers.IO) { services.apiKeys.deeplKey() }
            ?: return Result.failure(IllegalStateException(NO_DEEPL_KEY))
        val target = services.settings.settings.value.nativeLanguage
        return runCatching { repository.translate(text, context, book.languageCode, target, key).text }
            .recoverCatching { throw IllegalStateException(describe(it)) }
    }

    fun dismissWord() = _state.update { it.copy(selected = null) }

    /** Saves the selected word (or forgets it for New), then recolours the chapter. */
    fun saveWord(status: WordStatus, translation: String, notes: String) {
        val selected = _state.value.selected ?: return
        val state = _state.value
        val book = state.book ?: return
        val language = book.languageCode
        // The card reuses the sentence translation already on screen, so saving costs no quota.
        val sentenceTranslation = (state.translations[selected.rowKey] as? SentenceTranslation.Shown)?.text
        viewModelScope.launch {
            val result = runCatching {
                if (status == WordStatus.New) {
                    repository.forgetTerm(language, selected.text)
                } else {
                    repository.saveTerm(
                        TermInput(
                            languageCode = language,
                            text = selected.text,
                            status = status,
                            translation = translation.trim(),
                            notes = notes.trim(),
                            lemma = selected.saved?.lemma,
                            sentence = selected.sentence,
                            sentenceTranslation = sentenceTranslation,
                            sourceBook = book.title,
                            sourceChapter = state.chapterTitle,
                        ),
                    )
                }
            }
            _state.update { it.copy(selected = null, message = result.exceptionOrNull()?.let(::describe)) }
            reloadStatuses()
            if (result.isSuccess) services.syncAnki()
        }
    }

    /** PRD F8: marks every new word from `fromBlock` to the chapter's end as known. */
    fun markRestKnown(fromBlock: Int) {
        viewModelScope.launch {
            val result = runCatching { repository.markRestKnown(bookId, _state.value.chapterIndex, fromBlock.toUInt()) }
            _state.update { it.copy(message = result.fold({ n -> "Marked $n words as known" }, ::describe)) }
            reloadStatuses()
        }
    }

    fun messageShown() = _state.update { it.copy(message = null) }

    /** Reopens the chapter: the parse is cached, so only the statuses change. */
    private suspend fun reloadStatuses() {
        runCatching { repository.openChapter(bookId, _state.value.chapterIndex) }
            .onSuccess { chapter -> _state.update { it.copy(rows = readerRows(chapter.blocks)) } }
    }
}
