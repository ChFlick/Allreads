package io.github.chflick.allreads.ui.reader

import androidx.compose.material3.FilledTonalButton
import io.github.chflick.allreads.ui.settings.onMobileData
import androidx.compose.material3.AlertDialog
import io.github.chflick.allreads.data.DownloadState
import io.github.chflick.allreads.ui.megabytes
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.foundation.layout.PaddingValues
import uniffi.allreads_ffi.DictionaryEntry
import androidx.compose.material3.IconButton
import androidx.compose.material3.Icon
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.Icons
import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.Surface
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontStyle
import android.widget.Toast
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import io.github.chflick.allreads.ui.theme.StatusChip
import io.github.chflick.allreads.ui.theme.wordStatusChoices
import uniffi.allreads_ffi.WordStatus

/** What the drawer would save: the status and the three text fields. */
data class WordFields(val status: WordStatus, val translation: String, val notes: String, val lemma: String)

/**
 * The drawer's editable state. A status chip saves at once; text typed after
 * that is saved when the drawer closes, so nothing typed is lost.
 */
class WordDraft(word: SelectedWord) {
    // A first tap on a new word suggests level 1, the usual choice.
    var status by mutableStateOf(if (word.status == WordStatus.New) WordStatus.Learning(1u) else word.status)
    var translation by mutableStateOf(word.saved?.translation ?: "")
    var notes by mutableStateOf(word.saved?.notes ?: "")
    var lemma by mutableStateOf(word.saved?.lemma ?: "")
    /** What was last saved, or null while the word is unsaved. */
    var lastSaved: WordFields? = word.saved?.let { WordFields(word.status, it.translation, it.notes, it.lemma ?: "") }
    /** True once the user has typed in a field (not when DeepL or the dictionary filled it). */
    var typed by mutableStateOf(false)

    val fields get() = WordFields(status, translation, notes, lemma)

    /** What to save when the drawer closes: changes to a saved word, or a word the user typed into. */
    fun toSaveOnClose(): WordFields? {
        val saved = lastSaved
        return when {
            saved != null -> fields.takeIf { it != saved && it.status != WordStatus.New }
            typed -> fields.takeIf { it.status != WordStatus.New }
            else -> null
        }
    }
}

/**
 * Step 13: the word drawer. It opens half-height with the essentials (word,
 * translation in context, status, save); pull it up for the sentence, notes
 * and a web dictionary.
 */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun WordSheet(
    word: SelectedWord,
    onTranslate: () -> Unit,
    speakingWord: Boolean,
    onSpeak: () -> Unit,
    onShowLemma: (String) -> Unit,
    onDownloadDictionary: () -> Unit,
    dictionaryDownload: DownloadState?,
    onChoosePhrase: (PhraseChoice) -> Unit,
    onSave: (WordFields, close: Boolean) -> Unit,
    onDismiss: () -> Unit,
    hideTranslations: Boolean = false,
    showPhraseHint: Boolean = false,
    onPhraseHintSeen: () -> Unit = {},
) {
    val draft = remember(word.text) { WordDraft(word) }
    val close = { draft.toSaveOnClose()?.let { onSave(it, true) } ?: onDismiss() }
    ModalBottomSheet(onDismissRequest = close, sheetState = rememberModalBottomSheetState()) {
        WordSheetContent(
            word = word,
            onTranslate = onTranslate,
            speakingWord = speakingWord,
            onSpeak = onSpeak,
            onShowLemma = onShowLemma,
            onDownloadDictionary = onDownloadDictionary,
            dictionaryDownload = dictionaryDownload,
            onChoosePhrase = onChoosePhrase,
            onSave = { onSave(it, false) },
            draft = draft,
            hideTranslations = hideTranslations,
            showPhraseHint = showPhraseHint,
            onPhraseHintSeen = onPhraseHintSeen,
        )
    }
}

/** The drawer's contents, without the sheet around it, for previews and screenshots. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
fun WordSheetContent(
    word: SelectedWord,
    onTranslate: () -> Unit,
    speakingWord: Boolean,
    onSpeak: () -> Unit,
    onShowLemma: (String) -> Unit,
    onDownloadDictionary: () -> Unit,
    dictionaryDownload: DownloadState?,
    onChoosePhrase: (PhraseChoice) -> Unit,
    onSave: (WordFields) -> Unit,
    draft: WordDraft = remember(word.text) { WordDraft(word) },
    hideTranslations: Boolean = false,
    showPhraseHint: Boolean = false,
    onPhraseHintSeen: () -> Unit = {},
) {
    // With "meanings in the book's language" on, translations wait for a tap on Reveal.
    var revealed by rememberSaveable(word.text) { mutableStateOf(!hideTranslations) }
    // Prefilled from the dictionary's suggestion, once it arrives, unless the word already has one.
    LaunchedEffect(word.dictionary?.suggestedLemma) {
        val suggested = word.dictionary?.suggestedLemma
        if (suggested != null && draft.lemma.isBlank() && word.saved?.lemma == null) draft.lemma = suggested
    }
    // Fill an empty translation field with DeepL's answer when it arrives.
    LaunchedEffect(word.machineTranslation) {
        val machine = word.machineTranslation
        if (machine != null && draft.translation.isBlank()) draft.translation = machine
    }
    val translation = draft.translation
    val context = LocalContext.current

        Column(
        Modifier
            .fillMaxWidth()
            .verticalScroll(rememberScrollState())
            .padding(horizontal = 20.dp, vertical = 4.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        // --- What the half-open drawer shows ---
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(
                word.saved?.displayText ?: word.text,
                style = MaterialTheme.typography.headlineSmall,
                modifier = Modifier.weight(1f, fill = false),
            )
            IconButton(onClick = onSpeak) {
                Icon(
                    Icons.Default.PlayArrow,
                    contentDescription = "Say the word",
                    tint = if (speakingWord) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        // A chip saves straight away; "New" forgets a saved word.
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            wordStatusChoices.forEach { choice ->
                StatusChip(
                    status = choice,
                    selected = draft.lastSaved != null && draft.status == choice,
                    onClick = {
                        draft.status = choice
                        if (choice != WordStatus.New || draft.lastSaved != null) {
                            onSave(draft.fields)
                            draft.lastSaved = draft.fields.takeIf { choice != WordStatus.New }
                        }
                    },
                )
            }
        }
        // Shown until dismissed, or until a phrase is selected by dragging.
        if (showPhraseHint) {
            Surface(
                color = MaterialTheme.colorScheme.secondaryContainer,
                shape = MaterialTheme.shapes.medium,
                modifier = Modifier.fillMaxWidth(),
            ) {
                Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.padding(start = 12.dp)) {
                    Text(
                        "Tip: to save a phrase, long-press its first word and drag to its last.",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSecondaryContainer,
                        modifier = Modifier.weight(1f),
                    )
                    TextButton(onClick = onPhraseHintSeen) { Text("Got it") }
                }
            }
        }
        // Dictionary phrases around the word come first, so they show in the half-open drawer.
        word.phraseSuggestions.forEach { choice ->
            FilledTonalButton(onClick = { onChoosePhrase(choice) }, modifier = Modifier.fillMaxWidth()) {
                Text(
                    "Phrase: ${choice.suggestion.text}" +
                        if (choice.suggestion.gloss.isBlank() || !revealed) "" else " · ${choice.suggestion.gloss}",
                    maxLines = 2,
                )
            }
        }
        if (revealed) when {
            word.translating -> Text("Translating…", color = MaterialTheme.colorScheme.onSurfaceVariant)
            word.translationError != null -> Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    word.translationError,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.weight(1f),
                )
                TextButton(onClick = onTranslate) { Text("Retry") }
            }
            word.machineTranslation != null && word.machineTranslation != translation -> Text(
                "DeepL: ${word.machineTranslation}",
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }
        if (revealed) OutlinedTextField(
            value = translation,
            onValueChange = {
                draft.translation = it
                draft.typed = true
            },
            label = { Text("Translation") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        OfflineDictionary(word, dictionaryDownload, onShowLemma, onDownloadDictionary, revealed)
        if (!revealed) {
            FilledTonalButton(onClick = { revealed = true }, modifier = Modifier.fillMaxWidth()) {
                Text("Reveal translation")
            }
        }

        // --- Pull the drawer up for more context ---
        HorizontalDivider(Modifier.padding(top = 8.dp))
        Text("In this sentence", style = MaterialTheme.typography.labelLarge)
        Text(word.sentence, style = MaterialTheme.typography.bodyLarge)
        OutlinedTextField(
            value = draft.notes,
            onValueChange = {
                draft.notes = it
                draft.typed = true
            },
            label = { Text("Notes") },
            minLines = 2,
            modifier = Modifier.fillMaxWidth(),
        )
        OutlinedTextField(
            value = draft.lemma,
            onValueChange = {
                draft.lemma = it
                draft.typed = true
            },
            label = { Text("Dictionary form (lemma)") },
            singleLine = true,
            modifier = Modifier.fillMaxWidth(),
        )
        word.dictionaryUrl?.let { url ->
            OutlinedButton(
                onClick = {
                    try {
                        context.startActivity(Intent(Intent.ACTION_VIEW, Uri.parse(url)))
                    } catch (e: ActivityNotFoundException) {
                        Toast.makeText(context, "No browser is installed to open the dictionary.", Toast.LENGTH_LONG).show()
                    }
                },
                modifier = Modifier.fillMaxWidth().padding(bottom = 24.dp),
            ) { Text("Look up in the web dictionary") }
        }
    }
}

/** The offline dictionary's part of the drawer: forms, meanings, or a download offer. */
@Composable
private fun OfflineDictionary(
    word: SelectedWord,
    download: DownloadState?,
    onShowLemma: (String) -> Unit,
    onDownload: () -> Unit,
    revealed: Boolean,
) {
    val lookup = word.dictionary
    if (lookup == null) {
        val offer = word.dictionaryOffer ?: return
        val context = LocalContext.current
        var confirm by remember { mutableStateOf(false) }
        val start = { if (onMobileData(context)) confirm = true else onDownload() }
        if (confirm) {
            AlertDialog(
                onDismissRequest = { confirm = false },
                title = { Text("Download on mobile data?") },
                text = { Text("The dictionary is a ${megabytes(offer.downloadBytes.toLong())} download.") },
                confirmButton = {
                    TextButton(onClick = {
                        confirm = false
                        onDownload()
                    }) { Text("Download") }
                },
                dismissButton = { TextButton(onClick = { confirm = false }) { Text("Not now") } },
            )
        }
        when (download) {
            is DownloadState.Running -> Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                LinearProgressIndicator(
                    progress = { (download.bytes.toFloat() / download.total.coerceAtLeast(1)).coerceIn(0f, 1f) },
                    modifier = Modifier.fillMaxWidth(),
                )
                Text(
                    "Downloading the dictionary: ${megabytes(download.bytes)} of ${megabytes(download.total)}",
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            is DownloadState.Failed -> Column {
                Text(download.message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
                TextButton(onClick = start) { Text("Try the download again") }
            }
            null -> TextButton(onClick = start) {
                Text("Get the offline dictionary (${megabytes(offer.downloadBytes.toLong())})")
            }
        }
        return
    }
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        lookup.forms.forEach { form ->
            // "→ lire (verb): imperfect indicative singular third-person", tap for lire's entry.
            TextButton(onClick = { onShowLemma(form.lemma) }, contentPadding = PaddingValues(0.dp)) {
                Text(
                    "→ ${form.lemma} (${form.partOfSpeech})" + if (form.tags.isBlank()) "" else ": ${form.tags}",
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }
        lookup.entries.forEach { DictionaryEntryText(it, revealed) }
        word.lemmaEntry?.let { (lemma, entry) ->
            HorizontalDivider()
            Text(lemma, style = MaterialTheme.typography.titleMedium)
            entry.entries.forEach { DictionaryEntryText(it, revealed) }
        }
        if (lookup.entries.isEmpty() && lookup.forms.isEmpty()) {
            Text("Not in the offline dictionary.", style = MaterialTheme.typography.bodySmall)
        } else if (!revealed && (lookup.entries + word.lemmaEntry?.second?.entries.orEmpty()).all { it.synonyms.isEmpty() }) {
            Text("No synonyms in the dictionary.", style = MaterialTheme.typography.bodySmall)
        }
    }
}

/**
 * "noun · /li/", its synonyms, and up to three numbered meanings (tap "more"
 * for the rest). The meanings are English, so they wait for [revealed].
 */
@Composable
private fun DictionaryEntryText(entry: DictionaryEntry, revealed: Boolean) {
    var all by remember(entry) { mutableStateOf(false) }
    Text(
        listOf(entry.partOfSpeech, entry.ipa).filter { it.isNotBlank() }.joinToString(" · "),
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    if (entry.synonyms.isNotEmpty()) {
        Text(
            "≈ ${entry.synonyms.joinToString(", ")}",
            style = MaterialTheme.typography.bodyMedium,
            fontStyle = FontStyle.Italic,
        )
    }
    if (!revealed) return
    val shown = if (all) entry.senses else entry.senses.take(3)
    shown.forEachIndexed { index, sense ->
        Text("${index + 1}. $sense", style = MaterialTheme.typography.bodyMedium)
    }
    if (!all && entry.senses.size > 3) {
        TextButton(onClick = { all = true }, contentPadding = PaddingValues(0.dp)) {
            Text("${entry.senses.size - 3} more")
        }
    }
}

