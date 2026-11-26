package io.github.chflick.allreads.ui.words

import android.content.ContentResolver
import android.net.Uri
import android.provider.OpenableColumns
import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import io.github.chflick.allreads.anki.AnkiAccess
import io.github.chflick.allreads.anki.AnkiBridge
import io.github.chflick.allreads.anki.AnkiDeck
import io.github.chflick.allreads.anki.DeckImport
import io.github.chflick.allreads.data.Repository
import io.github.chflick.allreads.ui.describe
import java.io.File
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.allreads_ffi.ImportGroup
import uniffi.allreads_ffi.ImportRow
import uniffi.allreads_ffi.ImportStatus
import uniffi.allreads_ffi.ImportSummary
import uniffi.allreads_ffi.WordStatus

/** The fields picked in one group: which holds the word (null: skip the group) and the meaning. */
data class FieldChoice(val word: Int? = null, val meaning: Int? = null)

/** A sample note: the raw word field and what the import would make of it. */
data class Sample(val raw: String, val word: String?)

data class AnkiImportState(
    val access: AnkiAccess? = null,
    val decks: List<AnkiDeck> = emptyList(),
    val languages: List<String> = emptyList(),
    val language: String? = null,
    /** The badge imported words get, e.g. "Anki · French vocabulary". */
    val source: String? = null,
    /** From Anki (AnkiDroid or .apkg): the words have cards there already. */
    val fromAnki: Boolean = false,
    val groups: List<ImportGroup> = emptyList(),
    val choices: List<FieldChoice> = emptyList(),
    /** Samples per group, for its chosen word field. */
    val samples: Map<Int, List<Sample>> = emptyMap(),
    val followReviews: Boolean = false,
    val level: WordStatus = WordStatus.WellKnown,
    val reading: Boolean = false,
    val importing: Boolean = false,
    val summary: ImportSummary? = null,
    val error: String? = null,
) {
    /** Whether the notes carry review progress (AnkiDroid, .apkg), so "follow Anki" makes sense. */
    val hasProgress: Boolean get() = groups.any { group -> group.notes.any { it.progress != null } }
    val noteCount: Int get() = groups.zip(choices).filter { it.second.word != null }.sumOf { it.first.notes.size }
    val canImport: Boolean get() = language != null && noteCount > 0 && !importing && summary == null
}

/**
 * The "Import from Anki" screen: read an AnkiDroid deck or a file, pick the
 * word and meaning fields per note type, choose how statuses are set, import.
 * Cleaning words and the status rules happen in the core.
 */
class AnkiImportViewModel(
    private val repository: Repository,
    private val anki: AnkiBridge,
    language: String?,
    private val cacheDir: File,
) : ViewModel() {
    private val _state = MutableStateFlow(AnkiImportState(language = language))
    val state: StateFlow<AnkiImportState> = _state.asStateFlow()

    init {
        viewModelScope.launch {
            val access = withContext(Dispatchers.IO) { anki.access() }
            val decks = if (access == AnkiAccess.Connected) {
                runCatching { withContext(Dispatchers.IO) { anki.decks() } }.getOrDefault(emptyList())
            } else {
                emptyList()
            }
            val languages = runCatching { repository.languageCodes() }.getOrDefault(emptyList())
            _state.update {
                it.copy(access = access, decks = decks, languages = languages, language = it.language ?: languages.firstOrNull())
            }
        }
    }

    fun chooseLanguage(code: String) {
        _state.update { it.copy(language = code) }
        refreshSamples()
    }

    /** Reads an AnkiDroid deck (subdecks included). */
    fun readDeck(deck: AnkiDeck) = read(source = "Anki · ${deck.name}", fromAnki = true) {
        withContext(Dispatchers.IO) { DeckImport(anki).read(deck.name) }
    }

    /** Reads a picked file: an `.apkg` deck, or a text/CSV export. */
    fun readFile(resolver: ContentResolver, uri: Uri) {
        val name = resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
            ?.use { cursor -> if (cursor.moveToFirst()) cursor.getString(0) else null } ?: "import.txt"
        val isDeck = name.lowercase().let { it.endsWith(".apkg") || it.endsWith(".colpkg") }
        val label = if (isDeck) "Anki · ${name.substringBeforeLast('.')}" else "File · $name"
        read(source = label, fromAnki = isDeck) {
            val copy = File(cacheDir, "import-${System.currentTimeMillis()}")
            try {
                withContext(Dispatchers.IO) {
                    resolver.openInputStream(uri).use { input ->
                        requireNotNull(input) { "The file could not be opened" }
                        copy.outputStream().use { input.copyTo(it) }
                    }
                }
                repository.readImportFile(copy.absolutePath, name)
            } finally {
                withContext(Dispatchers.IO) { copy.delete() }
            }
        }
    }

    private fun read(source: String, fromAnki: Boolean, load: suspend () -> List<ImportGroup>) {
        _state.update { it.copy(reading = true, error = null, summary = null) }
        viewModelScope.launch {
            val result = runCatching { load() }
            _state.update { state ->
                result.fold(
                    { groups ->
                        state.copy(
                            reading = false,
                            source = source,
                            fromAnki = fromAnki,
                            groups = groups,
                            choices = groups.map { FieldChoice() },
                            samples = emptyMap(),
                            followReviews = groups.any { g -> g.notes.any { it.progress != null } },
                            error = if (groups.isEmpty()) "No notes found there." else null,
                        )
                    },
                    { state.copy(reading = false, error = describe(it)) },
                )
            }
        }
    }

    fun chooseWordField(group: Int, field: Int?) {
        updateChoice(group) { it.copy(word = field) }
        refreshSamples()
    }

    fun chooseMeaningField(group: Int, field: Int?) = updateChoice(group) { it.copy(meaning = field) }

    fun followReviews(on: Boolean) = _state.update { it.copy(followReviews = on) }

    fun chooseLevel(status: WordStatus) = _state.update { it.copy(level = status, followReviews = false) }

    private fun updateChoice(group: Int, change: (FieldChoice) -> FieldChoice) = _state.update { state ->
        state.copy(choices = state.choices.mapIndexed { i, choice -> if (i == group) change(choice) else choice })
    }

    /** Cleans the first three notes of each group's chosen word field, as the import would. */
    private fun refreshSamples() {
        val state = _state.value
        val language = state.language ?: return
        viewModelScope.launch {
            val samples = state.groups.zip(state.choices).mapIndexedNotNull { index, (group, choice) ->
                val field = choice.word ?: return@mapIndexedNotNull null
                index to group.notes.take(3).map { note ->
                    val raw = note.fields.getOrElse(field) { "" }
                    Sample(raw, runCatching { repository.cleanImportWord(language, raw) }.getOrNull())
                }
            }.toMap()
            _state.update { it.copy(samples = samples) }
        }
    }

    fun import() {
        val state = _state.value
        val language = state.language ?: return
        val rows = state.groups.zip(state.choices).flatMap { (group, choice) ->
            val word = choice.word ?: return@flatMap emptyList()
            group.notes.map { note ->
                ImportRow(
                    word = note.fields.getOrElse(word) { "" },
                    meaning = choice.meaning?.let { note.fields.getOrNull(it) },
                    progress = note.progress,
                )
            }
        }
        val status = if (state.followReviews && state.hasProgress) ImportStatus.FollowReviews else ImportStatus.Fixed(state.level)
        _state.update { it.copy(importing = true, error = null) }
        viewModelScope.launch {
            val result = runCatching {
                repository.importWords(language, state.source ?: "Import", state.fromAnki, rows, status)
            }
            _state.update {
                it.copy(importing = false, summary = result.getOrNull(), error = result.exceptionOrNull()?.let(::describe))
            }
        }
    }
}
