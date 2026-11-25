package io.github.chflick.allreads.ui.reader

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import io.github.chflick.allreads.data.Repository
import io.github.chflick.allreads.reader.BlockText
import io.github.chflick.allreads.ui.library.describe
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import uniffi.allreads_ffi.BookSummary
import uniffi.allreads_ffi.SavedTerm
import uniffi.allreads_ffi.TermInput
import uniffi.allreads_ffi.WordStatus

/** The word the reader tapped, with what the word sheet needs. */
data class SelectedWord(
    val text: String,
    val sentence: String,
    val status: WordStatus,
    val saved: SavedTerm?,
)

data class ReaderState(
    val book: BookSummary? = null,
    val chapterIndex: UInt = 0u,
    val chapterTitle: String? = null,
    val blocks: List<BlockText> = emptyList(),
    /** The block to scroll to when a chapter opens (the saved position). */
    val startBlock: Int = 0,
    val selected: SelectedWord? = null,
    val message: String? = null,
)

class ReaderViewModel(private val repository: Repository, private val bookId: Long) : ViewModel() {
    private val _state = MutableStateFlow(ReaderState())
    val state: StateFlow<ReaderState> = _state.asStateFlow()

    private var titles: List<String?> = emptyList()

    init {
        viewModelScope.launch {
            val book = repository.book(bookId)
            if (book == null) {
                _state.update { it.copy(message = "This book is no longer in the library") }
                return@launch
            }
            titles = runCatching { repository.chapterTitles(bookId) }.getOrDefault(emptyList())
            _state.update { it.copy(book = book) }
            load(book.chapterIndex, startBlock = book.paragraphIndex.toInt())
        }
    }

    private suspend fun load(index: UInt, startBlock: Int) {
        val result = runCatching { repository.openChapter(bookId, index) }
        result.onSuccess { chapter ->
            _state.update {
                it.copy(
                    chapterIndex = index,
                    chapterTitle = titles.getOrNull(index.toInt()),
                    blocks = chapter.blocks.map(::BlockText),
                    startBlock = startBlock,
                )
            }
        }
        result.onFailure { e -> _state.update { it.copy(message = describe(e)) } }
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
        viewModelScope.launch {
            runCatching { repository.savePosition(bookId, chapter, block.toUInt()) }
        }
    }

    fun select(blockIndex: Int, tokenIndex: Int) {
        val block = _state.value.blocks.getOrNull(blockIndex) ?: return
        val token = block.block.tokens.getOrNull(tokenIndex) ?: return
        val language = _state.value.book?.languageCode ?: return
        viewModelScope.launch {
            val saved = runCatching { repository.findTerm(language, token.text) }.getOrNull()
            _state.update {
                it.copy(
                    selected = SelectedWord(
                        text = token.text,
                        sentence = block.sentenceOf(tokenIndex),
                        status = token.status ?: WordStatus.New,
                        saved = saved,
                    ),
                )
            }
        }
    }

    fun dismissWord() = _state.update { it.copy(selected = null) }

    /** Saves the selected word (or forgets it for New), then recolours the chapter. */
    fun saveWord(status: WordStatus, translation: String) {
        val selected = _state.value.selected ?: return
        val language = _state.value.book?.languageCode ?: return
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
                            notes = selected.saved?.notes ?: "",
                            lemma = selected.saved?.lemma,
                        ),
                    )
                }
            }
            _state.update { it.copy(selected = null, message = result.exceptionOrNull()?.let(::describe)) }
            reloadStatuses()
        }
    }

    /** PRD F8: marks every new word from `fromBlock` to the chapter's end as known. */
    fun markRestKnown(fromBlock: Int) {
        viewModelScope.launch {
            val result = runCatching {
                repository.markRestKnown(bookId, _state.value.chapterIndex, fromBlock.toUInt())
            }
            val message = result.fold({ "Marked $it words as known" }, ::describe)
            _state.update { it.copy(message = message) }
            reloadStatuses()
        }
    }

    fun messageShown() = _state.update { it.copy(message = null) }

    /** Reopens the chapter: the parse is cached, so only the statuses change. */
    private suspend fun reloadStatuses() {
        val current = _state.value
        runCatching { repository.openChapter(bookId, current.chapterIndex) }
            .onSuccess { chapter -> _state.update { it.copy(blocks = chapter.blocks.map(::BlockText)) } }
    }
}
