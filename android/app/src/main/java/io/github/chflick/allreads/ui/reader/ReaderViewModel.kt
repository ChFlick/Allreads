package io.github.chflick.allreads.ui.reader

import uniffi.allreads_ffi.MarkedKnown
import uniffi.allreads_ffi.PhraseSuggestion
import io.github.chflick.allreads.reader.commonForm
import io.github.chflick.allreads.data.DownloadState
import uniffi.allreads_ffi.DictionaryLookup
import uniffi.allreads_ffi.AvailableDictionary
import io.github.chflick.allreads.speech.SpeakResult
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
    /** The offline dictionary's entry; `null` when no dictionary is installed for the language. */
    val dictionary: DictionaryLookup? = null,
    /** A dictionary that could be downloaded, when none is installed. */
    val dictionaryOffer: AvailableDictionary? = null,
    /** The entry of a dictionary form tapped in the drawer, e.g. "lire" for "lisait". */
    val lemmaEntry: Pair<String, DictionaryLookup>? = null,
    /** True for a phrase of several words. */
    val isPhrase: Boolean = false,
    /** Dictionary phrases around a tapped word, e.g. "pierre précieuse", to save instead. */
    val phraseSuggestions: List<PhraseChoice> = emptyList(),
)

/** A word's level before and after a double tap. */
data class LevelChange(val text: String, val from: WordStatus, val to: WordStatus)

/** A dictionary phrase in the tapped word's sentence, and the tokens it covers in the row. */
data class PhraseChoice(val suggestion: PhraseSuggestion, val tokens: IntRange)

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
    /** A level just raised by a double tap, offered for undo in a snackbar. */
    val levelChange: LevelChange? = null,
    /** Words just marked known, offered for undo in a snackbar. */
    val markedKnown: MarkedKnown? = null,
    /** Set when read-aloud found no voice for the book's language; the screen offers the TTS settings. */
    val noVoice: Boolean = false,
)

/**
 * The reader's state. One instance serves every book: [open] starts a fresh
 * session for each book opening, while a rotation keeps the current one.
 */
/** The utterance id for the word in the drawer. */
const val WORD_UTTERANCE = "word"

class ReaderViewModel(private val services: Services) : ViewModel() {
    private val repository = services.repository
    private val _state = MutableStateFlow(ReaderState())
    val state: StateFlow<ReaderState> = _state.asStateFlow()

    private var bookId: Long = -1
    private var openId: Int = -1
    private var titles: List<String?> = emptyList()
    private var dictionaryTemplate: String = ""
    /** The book language's text-to-speech locale, e.g. `fr-FR`. */
    private var ttsLocale: String = ""

    /** What is being read aloud: a row key, [WORD_UTTERANCE], or `null`. */
    val speaking: StateFlow<String?> = services.speech.speaking

    /** Running dictionary downloads, so the drawer can show progress. */
    val dictionaryDownloads: StateFlow<Map<String, DownloadState>> = services.dictionaries.downloads
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
        ttsLocale = ""
        services.speech.stop()
        _state.value = ReaderState()
        openJob = viewModelScope.launch {
            val book = repository.book(bookId)
            if (book == null) {
                _state.update { it.copy(message = "This book is no longer in the library") }
                return@launch
            }
            titles = runCatching { repository.chapterTitles(bookId) }.getOrDefault(emptyList())
            val language = runCatching { repository.languageInfo(book.languageCode) }.getOrNull()
            dictionaryTemplate = language?.dictionaryUrl.orEmpty()
            ttsLocale = language?.ttsLocale?.ifBlank { null } ?: book.languageCode
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

    /** Asks DeepL again for a sentence whose translation failed. */
    fun retryTranslation(row: ReaderRow) {
        _state.update { it.copy(translations = it.translations - row.key) }
        toggleTranslation(row)
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

    /** Opens the drawer for a tapped word, or for the saved phrase it is part of. */
    fun select(row: ReaderRow, tokenIndex: Int) {
        val token = row.content.tokens.getOrNull(tokenIndex) ?: return
        row.content.phraseRange(tokenIndex)?.let { phrase ->
            selectRange(row, phrase)
            return
        }
        open(row, token.text, token.status ?: WordStatus.New, isPhrase = false) { language ->
            suggestPhrases(row, tokenIndex, language)
        }
    }

    /** Opens the drawer for the words in `tokens` (a phrase), e.g. after a long-press and drag. */
    fun selectRange(row: ReaderRow, tokens: IntRange) {
        val words = tokens.filter { row.content.tokens[it].termKey != null }
        if (words.size <= 1) {
            words.firstOrNull()?.let { select(row, it) }
            return
        }
        // They found the gesture; the drawer's tip about it can go.
        phraseHintSeen()
        val status = row.content.tokens[tokens.first].phrase?.status ?: WordStatus.New
        open(row, row.content.textOf(tokens), status, isPhrase = true) {}
    }

    private fun open(
        row: ReaderRow,
        text: String,
        status: WordStatus,
        isPhrase: Boolean,
        more: suspend (language: String) -> Unit,
    ) {
        val language = _state.value.book?.languageCode ?: return
        // A newer tap wins: an older lookup that finishes later must not replace it.
        selectJob?.cancel()
        selectJob = viewModelScope.launch {
            val saved = runCatching { repository.findTerm(language, text) }.getOrNull()
            val selected = SelectedWord(
                text = text,
                sentence = row.sentence,
                rowKey = row.key,
                status = saved?.status ?: status,
                saved = saved,
                dictionaryUrl = dictionaryLink(dictionaryTemplate, text),
                isPhrase = isPhrase,
            )
            _state.update { it.copy(selected = selected) }
            if (services.settings.settings.value.speakOnTap) speakWord()
            lookUpSelected(language, text)
            more(language)
            // Unsaved (or untranslated) words get DeepL's translation in context.
            if (saved?.translation.isNullOrBlank()) translateSelected()
        }
    }

    /** Adds the dictionary's phrases around the tapped word to the drawer. */
    private suspend fun suggestPhrases(row: ReaderRow, tokenIndex: Int, language: String) {
        val wordTokens = row.content.wordIndexes
        val position = wordTokens.indexOf(tokenIndex).takeIf { it >= 0 } ?: return
        val words = wordTokens.map { row.content.tokens[it].text }
        val suggestions = runCatching {
            repository.phraseSuggestions(language, words, position.toUInt())
        }.getOrDefault(emptyList())
        val choices = suggestions.map {
            PhraseChoice(it, wordTokens[it.firstWord.toInt()]..wordTokens[it.lastWord.toInt()])
        }
        val text = row.content.tokens[tokenIndex].text
        _state.update { state ->
            val current = state.selected?.takeIf { it.text == text } ?: return@update state
            state.copy(selected = current.copy(phraseSuggestions = choices))
        }
    }

    /** Switches the drawer from the tapped word to a suggested phrase. */
    fun choosePhrase(choice: PhraseChoice) {
        val rowKey = _state.value.selected?.rowKey ?: return
        val row = _state.value.rows.firstOrNull { it.key == rowKey } ?: return
        selectRange(row, choice.tokens)
    }

    /** Adds the offline dictionary's entry, or a download offer, to the drawer. */
    private suspend fun lookUpSelected(language: String, text: String) {
        val lookup = runCatching { repository.lookupWord(language, text) }.getOrNull()
        val dictionaries = services.dictionaries
        if (lookup == null && dictionaries.list.value.available == null) {
            // Fetch the list in the background, so the tap's translation never waits on it.
            viewModelScope.launch {
                runCatching { dictionaries.refresh() }
                val offer = dictionaries.availableFor(language) ?: return@launch
                _state.update { state ->
                    val current = state.selected?.takeIf { it.text == text && it.dictionary == null }
                        ?: return@update state
                    state.copy(selected = current.copy(dictionaryOffer = offer))
                }
            }
        }
        val offer = if (lookup == null) dictionaries.availableFor(language) else null
        _state.update { state ->
            val current = state.selected?.takeIf { it.text == text } ?: return@update state
            state.copy(selected = current.copy(dictionary = lookup, dictionaryOffer = offer))
        }
    }

    /** Shows a dictionary form's entry in the drawer (tapping "lire" under "lisait"). */
    fun showLemma(lemma: String) {
        val word = _state.value.selected ?: return
        val language = _state.value.book?.languageCode ?: return
        viewModelScope.launch {
            val lookup = runCatching { repository.lookupWord(language, lemma) }.getOrNull() ?: return@launch
            _state.update { state ->
                val current = state.selected?.takeIf { it.text == word.text } ?: return@update state
                state.copy(selected = current.copy(lemmaEntry = lemma to lookup))
            }
        }
    }

    /** Downloads the offered dictionary in the background; the drawer updates when it is installed. */
    fun downloadDictionary() {
        val offer = _state.value.selected?.dictionaryOffer ?: return
        services.dictionaries.download(offer)
        viewModelScope.launch {
            // Wait for this download to end, whether it worked or not.
            services.dictionaries.awaitDownload(offer.pair)
            if (services.dictionaries.list.value.installed.none { it.pair == offer.pair }) return@launch
            val word = _state.value.selected ?: return@launch
            val language = _state.value.book?.languageCode ?: return@launch
            lookUpSelected(language, word.text)
        }
    }

    /** Fetches DeepL's translation of the selected word, with its sentence as context. */
    fun translateSelected() {
        val word = _state.value.selected ?: return
        _state.update { it.copy(selected = word.copy(translating = true, translationError = null)) }
        viewModelScope.launch {
            // The dictionary's entry was added before this runs, when one is installed.
            val text = commonForm(word.text, _state.value.selected?.dictionary)
            val result = translate(text, context = word.sentence)
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

    /**
     * Saves the selected word (or forgets it for New), then recolours the chapter.
     * A status chip saves with the drawer open; closing it saves late edits and closes.
     */
    fun saveWord(fields: WordFields, close: Boolean) {
        val (status, translation, notes, lemma) = fields
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
                            // "Coq" at a sentence start is saved as "coq", if the dictionary knows it as a common word.
                            text = commonForm(selected.text, selected.dictionary),
                            status = status,
                            translation = translation.trim(),
                            notes = notes.trim(),
                            lemma = lemma.trim().ifEmpty { null },
                            sentence = selected.sentence,
                            sentenceTranslation = sentenceTranslation,
                            sourceBook = book.title,
                            sourceChapter = state.chapterTitle,
                        ),
                    )
                }
            }
            _state.update { state ->
                state.copy(
                    selected = if (close) null else state.selected,
                    message = result.exceptionOrNull()?.let(::describe),
                )
            }
            reloadStatuses()
            if (result.isSuccess) services.syncAnki()
        }
    }

    /** PRD F8: marks every new word from `fromBlock` to the chapter's end as known. */
    fun markRestKnown(fromBlock: Int) {
        viewModelScope.launch {
            val result = runCatching { repository.markRestKnown(bookId, _state.value.chapterIndex, fromBlock.toUInt()) }
            _state.update { state ->
                result.fold(
                    // Nothing to undo when no word was new.
                    { marked -> if (marked.count == 0u) state.copy(message = "No new words left in this chapter") else state.copy(markedKnown = marked) },
                    { state.copy(message = describe(it)) },
                )
            }
            reloadStatuses()
        }
    }

    fun markedKnownShown() = _state.update { it.copy(markedKnown = null) }

    /**
     * A double tap on a learning word (or saved phrase) raises it one level,
     * from 5 to known, without opening the drawer. Other words open the
     * drawer, as a single tap would.
     */
    fun raiseLevel(row: ReaderRow, tokenIndex: Int) {
        val token = row.content.tokens.getOrNull(tokenIndex) ?: return
        val phrase = row.content.phraseRange(tokenIndex)
        val text = phrase?.let { row.content.textOf(it) } ?: token.text
        val status = token.phrase?.status ?: token.status
        val raised = (status as? WordStatus.Learning)?.let { learning ->
            if (learning.level < 5u) WordStatus.Learning((learning.level + 1u).toUByte()) else WordStatus.WellKnown
        }
        if (raised == null) {
            select(row, tokenIndex)
            return
        }
        changeLevel(LevelChange(text, from = status, to = raised))
    }

    fun levelChangeShown() = _state.update { it.copy(levelChange = null) }

    fun undoLevelChange(change: LevelChange) = changeLevel(change.copy(from = change.to, to = change.from), offerUndo = false)

    /** Saves a word at a new level, keeping its translation, notes and card sentence. */
    private fun changeLevel(change: LevelChange, offerUndo: Boolean = true) {
        val language = _state.value.book?.languageCode ?: return
        viewModelScope.launch {
            val result = runCatching {
                val saved = repository.findTerm(language, change.text) ?: error("“${change.text}” is not saved")
                repository.saveTerm(
                    TermInput(
                        languageCode = language,
                        text = saved.displayText,
                        status = change.to,
                        translation = saved.translation,
                        notes = saved.notes,
                        lemma = saved.lemma,
                        // No context: the card keeps the sentence it was saved with.
                        sentence = null,
                        sentenceTranslation = null,
                        sourceBook = null,
                        sourceChapter = null,
                    ),
                )
            }
            _state.update { state ->
                result.fold(
                    { if (offerUndo) state.copy(levelChange = change) else state },
                    { state.copy(message = describe(it)) },
                )
            }
            reloadStatuses()
            if (result.isSuccess) services.syncAnki()
        }
    }

    /** Puts the words from the last "mark the rest as known" back to new. */
    fun undoMarkKnown(marked: MarkedKnown) {
        viewModelScope.launch {
            val result = runCatching { repository.undoMarkKnown(marked.termIds) }
            result.exceptionOrNull()?.let { error -> _state.update { it.copy(message = describe(error)) } }
            reloadStatuses()
        }
    }

    /** The drawer's long-press tip was dismissed, or a phrase was selected: don't show it again. */
    fun phraseHintSeen() {
        if (!services.settings.settings.value.phraseHintSeen) services.settings.update { it.copy(phraseHintSeen = true) }
    }

    fun messageShown() = _state.update { it.copy(message = null) }

    fun noVoiceShown() = _state.update { it.copy(noVoice = false) }

    /** Reads a sentence aloud, or stops it if it is the one playing. */
    fun toggleSpeaking(row: ReaderRow) {
        if (speaking.value == row.key) {
            services.speech.stop()
        } else {
            speak(row.sentence, row.key)
        }
    }

    /** Says the word in the drawer. */
    fun speakWord() {
        val word = _state.value.selected ?: return
        speak(word.saved?.displayText ?: word.text, WORD_UTTERANCE)
    }

    fun stopSpeaking() = services.speech.stop()

    private fun speak(text: String, id: String) {
        val book = _state.value.book ?: return
        val settings = services.settings.settings.value
        viewModelScope.launch {
            val result = services.speech.speak(
                text = text,
                localeTag = ttsLocale,
                voiceName = settings.voices[book.languageCode],
                rate = settings.speechRate,
                id = id,
            )
            when (result) {
                SpeakResult.Started -> Unit
                SpeakResult.NoVoice -> _state.update { it.copy(noVoice = true) }
                SpeakResult.Unavailable -> _state.update { it.copy(message = "Text-to-speech is not available on this phone") }
            }
        }
    }

    override fun onCleared() = services.speech.stop()

    /** Reopens the chapter: the parse is cached, so only the statuses change. */
    private suspend fun reloadStatuses() {
        runCatching { repository.openChapter(bookId, _state.value.chapterIndex) }
            .onSuccess { chapter -> _state.update { it.copy(rows = readerRows(chapter.blocks)) } }
    }
}
