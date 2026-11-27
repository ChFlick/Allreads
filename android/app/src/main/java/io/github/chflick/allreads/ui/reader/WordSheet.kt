package io.github.chflick.allreads.ui.reader

import io.github.chflick.allreads.ui.AppText
import androidx.compose.ui.res.stringResource
import io.github.chflick.allreads.R
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.width
import androidx.compose.material3.FilledTonalIconButton
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.AnnotatedString
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.sp
import io.github.chflick.allreads.ui.StopIcon
import io.github.chflick.allreads.ui.theme.LevelBar
import io.github.chflick.allreads.ui.theme.Literata
import io.github.chflick.allreads.ui.theme.LocalDarkTheme
import io.github.chflick.allreads.ui.theme.highlight
import androidx.compose.material3.FilledTonalButton
import io.github.chflick.allreads.ui.settings.onMobileData
import androidx.compose.material3.AlertDialog
import io.github.chflick.allreads.data.DownloadState
import io.github.chflick.allreads.ui.megabytes
import io.github.chflick.allreads.ui.SourceBadge
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.foundation.layout.PaddingValues
import uniffi.allreads_ffi.DictionaryEntry
import androidx.compose.material3.IconButton
import androidx.compose.material3.Icon
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Edit
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.ui.focus.FocusRequester
import androidx.compose.ui.focus.focusRequester
import androidx.compose.ui.text.input.ImeAction
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
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalInspectionMode
import androidx.compose.ui.unit.dp
import io.github.chflick.allreads.ui.theme.StatusChip
import io.github.chflick.allreads.ui.theme.longLabel
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
        // --- What the half-open drawer shows: the word, its meaning, its level ---
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(10.dp)) {
            Text(
                word.saved?.displayText ?: word.text,
                style = MaterialTheme.typography.headlineMedium,
                modifier = Modifier.weight(1f, fill = false),
            )
            // Pronunciation and part of speech, from the dictionary's first entry.
            word.dictionary?.entries?.firstOrNull()?.let { entry ->
                Text(
                    listOf(entry.ipa, entry.partOfSpeech).filter { it.isNotBlank() }.joinToString(" · "),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 1,
                    modifier = Modifier.weight(1f, fill = false),
                )
            }
            Spacer(Modifier.weight(1f))
            FilledTonalIconButton(onClick = onSpeak) {
                Icon(
                    if (speakingWord) StopIcon else Icons.Default.PlayArrow,
                    contentDescription = if (speakingWord) AppText.get(R.string.stop) else AppText.get(R.string.say_the_word),
                )
            }
        }
        // Where an imported word came from, e.g. "Anki · French vocabulary".
        word.saved?.importedFrom?.let { SourceBadge(it) }

        // The meaning comes first: it is what the drawer is opened for.
        var editing by rememberSaveable(word.text) { mutableStateOf(false) }
        if (!revealed) {
            FilledTonalButton(onClick = { revealed = true }, modifier = Modifier.fillMaxWidth()) {
                Text(stringResource(R.string.reveal_translation))
            }
        } else if (editing) {
            val focus = remember { FocusRequester() }
            LaunchedEffect(Unit) { focus.requestFocus() }
            OutlinedTextField(
                value = translation,
                onValueChange = {
                    draft.translation = it
                    draft.typed = true
                },
                label = { Text(stringResource(R.string.translation)) },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Done),
                keyboardActions = KeyboardActions(onDone = { editing = false }),
                modifier = Modifier.fillMaxWidth().focusRequester(focus),
            )
        } else {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    translation.ifBlank { if (word.translating) AppText.get(R.string.translating) else AppText.get(R.string.no_translation_yet) },
                    style = MaterialTheme.typography.titleLarge.copy(fontFamily = FontFamily.Default, fontWeight = FontWeight.Normal, fontSize = 19.sp),
                    color = if (translation.isBlank()) MaterialTheme.colorScheme.onSurfaceVariant else MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.weight(1f),
                )
                IconButton(onClick = { editing = true }) {
                    Icon(Icons.Default.Edit, contentDescription = stringResource(R.string.edit_the_translation), tint = MaterialTheme.colorScheme.onSurfaceVariant)
                }
            }
        }
        if (revealed) when {
            word.translationError != null -> Row(verticalAlignment = Alignment.CenterVertically) {
                Text(
                    word.translationError,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.error,
                    modifier = Modifier.weight(1f),
                )
                TextButton(onClick = onTranslate) { Text(stringResource(R.string.retry)) }
            }
            word.machineTranslation != null && word.machineTranslation != translation && translation.isNotBlank() -> Text(
                stringResource(R.string.deepl, word.machineTranslation),
                style = MaterialTheme.typography.bodySmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
            )
        }

        // One row of levels; a tap saves straight away, "New" forgets a saved word.
        // The line under it confirms what a tap just did.
        var justSaved by remember(word.text) { mutableStateOf<WordStatus?>(null) }
        LevelBar(
            selected = draft.status.takeIf { draft.lastSaved != null },
            onSelect = { choice ->
                draft.status = choice
                if (choice != WordStatus.New || draft.lastSaved != null) {
                    justSaved = choice
                    onSave(draft.fields)
                    draft.lastSaved = draft.fields.takeIf { choice != WordStatus.New }
                }
            },
        )
        justSaved?.let { saved ->
            Text(
                if (saved == WordStatus.New) AppText.get(R.string.forgotten) else AppText.get(R.string.saved_as, saved.longLabel().lowercase()),
                style = MaterialTheme.typography.labelMedium,
                color = MaterialTheme.colorScheme.primary,
            )
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
                        stringResource(R.string.tip_to_save_a_phrase),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSecondaryContainer,
                        modifier = Modifier.weight(1f),
                    )
                    TextButton(onClick = onPhraseHintSeen) { Text(stringResource(R.string.got_it)) }
                }
            }
        }
        // Dictionary phrases around the word, as quiet rows: visible, but not louder than the word.
        word.phraseSuggestions.forEach { choice -> PhraseRow(choice, showGloss = revealed, onClick = { onChoosePhrase(choice) }) }
        OfflineDictionary(word, dictionaryDownload, onShowLemma, onDownloadDictionary, revealed)

        // --- Pull the drawer up for more context ---
        Text(stringResource(R.string.in_this_sentence), style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(top = 4.dp))
        val dark = LocalDarkTheme.current
        val mark = (draft.status.takeIf { draft.lastSaved != null } ?: WordStatus.New).highlight(dark)
            ?: MaterialTheme.colorScheme.surfaceContainerHighest
        Surface(color = MaterialTheme.colorScheme.surface, shape = MaterialTheme.shapes.medium) {
            Text(
                sentenceWithWord(word.sentence, word.text, mark),
                style = MaterialTheme.typography.bodyLarge.copy(fontFamily = Literata, lineHeight = 24.sp),
                modifier = Modifier.fillMaxWidth().padding(12.dp),
            )
        }
        Column {
            EditableRow(AppText.get(R.string.notes), draft.notes, placeholder = AppText.get(R.string.add_a_note), singleLine = false) {
                draft.notes = it
                draft.typed = true
            }
            EditableRow(AppText.get(R.string.dictionary_form), draft.lemma, placeholder = AppText.get(R.string.add_one)) {
                draft.lemma = it
                draft.typed = true
            }
        }
        // Shown only when an app can open the link (a phone without a browser has none).
        val preview = LocalInspectionMode.current
        val webLookup = word.dictionaryUrl?.let { url -> Intent(Intent.ACTION_VIEW, Uri.parse(url)) }
            ?.takeIf { intent -> preview || remember(intent.data) { intent.resolveActivity(context.packageManager) != null } }
        webLookup?.let { intent ->
            OutlinedButton(
                onClick = {
                    // The browser could have been removed since the drawer opened.
                    runCatching { context.startActivity(intent) }
                },
                modifier = Modifier.fillMaxWidth().padding(bottom = 24.dp),
            ) { Text(stringResource(R.string.look_up_in_the_web)) }
        }
    }
}

/** The sentence with the word marked in its level colour, as on the Anki card. */
private fun sentenceWithWord(sentence: String, word: String, colour: Color): AnnotatedString = buildAnnotatedString {
    val at = sentence.indexOf(word, ignoreCase = true)
    if (at < 0) {
        append(sentence)
        return@buildAnnotatedString
    }
    append(sentence.substring(0, at))
    withStyle(SpanStyle(background = colour)) { append(sentence.substring(at, at + word.length)) }
    append(sentence.substring(at + word.length))
}

/** A dictionary phrase around the word: "Part of perle rare · rare gem   Save ›". */
@Composable
private fun PhraseRow(choice: PhraseChoice, showGloss: Boolean, onClick: () -> Unit) {
    Surface(
        onClick = onClick,
        shape = MaterialTheme.shapes.small,
        color = MaterialTheme.colorScheme.secondaryContainer.copy(alpha = 0.35f),
        border = BorderStroke(1.dp, MaterialTheme.colorScheme.secondary.copy(alpha = 0.45f)),
        modifier = Modifier.fillMaxWidth(),
    ) {
        Row(Modifier.padding(horizontal = 12.dp, vertical = 9.dp), verticalAlignment = Alignment.CenterVertically) {
            Text(
                buildAnnotatedString {
                    append(AppText.get(R.string.part_of))
                    withStyle(SpanStyle(fontWeight = FontWeight.SemiBold)) { append(choice.suggestion.text) }
                    if (showGloss && choice.suggestion.gloss.isNotBlank()) append(" · ${choice.suggestion.gloss}")
                },
                style = MaterialTheme.typography.bodyMedium,
                maxLines = 2,
                modifier = Modifier.weight(1f),
            )
            Text(stringResource(R.string.save), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.secondary)
        }
    }
}

/** "Notes   Add a note   ✎": a value that opens for editing in place, like the translation. */
@Composable
private fun EditableRow(label: String, value: String, placeholder: String, singleLine: Boolean = true, onChange: (String) -> Unit) {
    var editing by rememberSaveable(label) { mutableStateOf(false) }
    HorizontalDivider(color = MaterialTheme.colorScheme.outlineVariant.copy(alpha = 0.6f))
    if (editing) {
        val focus = remember { FocusRequester() }
        LaunchedEffect(Unit) { focus.requestFocus() }
        OutlinedTextField(
            value = value,
            onValueChange = onChange,
            label = { Text(label) },
            singleLine = singleLine,
            minLines = if (singleLine) 1 else 2,
            keyboardOptions = KeyboardOptions(imeAction = if (singleLine) ImeAction.Done else ImeAction.Default),
            keyboardActions = KeyboardActions(onDone = { editing = false }),
            modifier = Modifier.fillMaxWidth().padding(vertical = 6.dp).focusRequester(focus),
        )
    } else {
        Row(
            Modifier.fillMaxWidth().clickable { editing = true }.padding(vertical = 10.dp),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(label, style = MaterialTheme.typography.bodyMedium, color = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.width(118.dp))
            Text(
                value.ifBlank { placeholder },
                style = MaterialTheme.typography.bodyMedium,
                fontStyle = if (value.isBlank()) FontStyle.Italic else null,
                color = if (value.isBlank()) MaterialTheme.colorScheme.outline else MaterialTheme.colorScheme.onSurface,
                modifier = Modifier.weight(1f),
            )
            Icon(Icons.Default.Edit, contentDescription = stringResource(R.string.edit, label), tint = MaterialTheme.colorScheme.onSurfaceVariant, modifier = Modifier.size(18.dp))
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
                title = { Text(stringResource(R.string.download_on_mobile_data)) },
                text = { Text(stringResource(R.string.the_dictionary_is_a_download, megabytes(offer.downloadBytes.toLong()))) },
                confirmButton = {
                    TextButton(onClick = {
                        confirm = false
                        onDownload()
                    }) { Text(stringResource(R.string.download)) }
                },
                dismissButton = { TextButton(onClick = { confirm = false }) { Text(stringResource(R.string.not_now)) } },
            )
        }
        when (download) {
            is DownloadState.Running -> Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
                LinearProgressIndicator(
                    progress = { (download.bytes.toFloat() / download.total.coerceAtLeast(1)).coerceIn(0f, 1f) },
                    modifier = Modifier.fillMaxWidth(),
                )
                Text(
                    stringResource(R.string.downloading_the_dictionary_of, megabytes(download.bytes), megabytes(download.total)),
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            is DownloadState.Failed -> Column {
                Text(download.message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
                TextButton(onClick = start) { Text(stringResource(R.string.try_the_download_again)) }
            }
            null -> TextButton(onClick = start) {
                Text(stringResource(R.string.get_the_offline_dictionary, megabytes(offer.downloadBytes.toLong())))
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
        // The first entry's part of speech and pronunciation are in the drawer's header already.
        lookup.entries.forEachIndexed { i, entry -> DictionaryEntryText(entry, revealed, showHeader = i > 0) }
        word.lemmaEntry?.let { (lemma, entry) ->
            HorizontalDivider()
            Text(lemma, style = MaterialTheme.typography.titleMedium)
            entry.entries.forEach { DictionaryEntryText(it, revealed) }
        }
        if (lookup.entries.isEmpty() && lookup.forms.isEmpty()) {
            Text(stringResource(R.string.not_in_the_offline_dictionary), style = MaterialTheme.typography.bodySmall)
        } else if (!revealed && (lookup.entries + word.lemmaEntry?.second?.entries.orEmpty()).all { it.synonyms.isEmpty() }) {
            Text(stringResource(R.string.no_synonyms_in_the_dictionary), style = MaterialTheme.typography.bodySmall)
        }
    }
}

/**
 * "noun · /li/", its synonyms, and up to three numbered meanings (tap "more"
 * for the rest). The meanings are English, so they wait for [revealed].
 */
@Composable
private fun DictionaryEntryText(entry: DictionaryEntry, revealed: Boolean, showHeader: Boolean = true) {
    var all by remember(entry) { mutableStateOf(false) }
    if (showHeader) Text(
        listOf(entry.partOfSpeech, entry.ipa).filter { it.isNotBlank() }.joinToString(" · "),
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
    if (entry.synonyms.isNotEmpty()) {
        Text(
            stringResource(R.string.text_3, entry.synonyms.joinToString(", ")),
            style = MaterialTheme.typography.bodyMedium,
            fontStyle = FontStyle.Italic,
        )
    }
    if (!revealed) return
    val shown = if (all) entry.senses else entry.senses.take(3)
    shown.forEachIndexed { index, sense ->
        Text(stringResource(R.string.text_4, index + 1, sense), style = MaterialTheme.typography.bodyMedium)
    }
    if (!all && entry.senses.size > 3) {
        TextButton(onClick = { all = true }, contentPadding = PaddingValues(0.dp)) {
            Text(stringResource(R.string.more, entry.senses.size - 3))
        }
    }
}

