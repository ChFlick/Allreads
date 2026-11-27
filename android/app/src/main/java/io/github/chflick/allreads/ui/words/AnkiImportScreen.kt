package io.github.chflick.allreads.ui.words

import androidx.compose.ui.res.stringResource
import io.github.chflick.allreads.R
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.compose.ui.window.Dialog
import androidx.compose.ui.window.DialogProperties
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.chflick.allreads.anki.AnkiAccess
import io.github.chflick.allreads.ui.languageName
import io.github.chflick.allreads.ui.settings.SettingsDropdown
import io.github.chflick.allreads.ui.theme.StatusChip
import io.github.chflick.allreads.ui.theme.wordStatusChoices
import uniffi.allreads_ffi.ImportGroup
import uniffi.allreads_ffi.WordStatus

/** "Import from Anki", full screen: source, fields per note type, status, then the summary. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun AnkiImportScreen(viewModel: AnkiImportViewModel, onClose: () -> Unit) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    val resolver = LocalContext.current.contentResolver
    val picker = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) viewModel.readFile(resolver, uri)
    }
    Dialog(onDismissRequest = onClose, properties = DialogProperties(usePlatformDefaultWidth = false)) {
        Scaffold(
            topBar = {
                TopAppBar(
                    title = { Text(stringResource(R.string.import_from_anki_2)) },
                    navigationIcon = {
                        IconButton(onClick = onClose) { Icon(Icons.Default.Close, contentDescription = stringResource(R.string.close)) }
                    },
                )
            },
        ) { padding ->
            Column(
                Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                ImportSourceSection(state, onDeck = viewModel::readDeck, onFile = {
                    // Phones report .apkg files under many types (or none): offer every file; the core refuses what it cannot read.
                    picker.launch(arrayOf("*/*"))
                })
                if (state.reading) LinearProgressIndicator(Modifier.fillMaxWidth())
                state.error?.let { Text(it, color = MaterialTheme.colorScheme.error) }
                if (state.groups.isNotEmpty() && state.summary == null) {
                    HorizontalDivider()
                    SettingsDropdown(
                        label = "Language of the words",
                        selected = state.language?.let(::languageName) ?: "",
                        options = state.languages.map { languageName(it) to it },
                        onSelect = viewModel::chooseLanguage,
                    )
                    Text(stringResource(R.string.which_field_holds_the_word), style = MaterialTheme.typography.titleSmall)
                    state.groups.forEachIndexed { index, group ->
                        GroupCard(
                            group = group,
                            choice = state.choices.getOrNull(index) ?: FieldChoice(),
                            samples = state.samples[index].orEmpty(),
                            onWord = { viewModel.chooseWordField(index, it) },
                            onMeaning = { viewModel.chooseMeaningField(index, it) },
                        )
                    }
                    StatusSection(state, onFollow = viewModel::followReviews, onLevel = viewModel::chooseLevel)
                    if (state.importing) LinearProgressIndicator(Modifier.fillMaxWidth())
                    Button(onClick = viewModel::import, enabled = state.canImport, modifier = Modifier.fillMaxWidth()) {
                        Text(stringResource(R.string.import_notes, "%,d".format(state.noteCount)))
                    }
                }
                state.summary?.let { summary ->
                    Text(stringResource(R.string.done), style = MaterialTheme.typography.titleMedium)
                    Text(
                        "${summary.added} words added · ${summary.upgraded} upgraded · " +
                            "${summary.unchanged} already known as well · ${summary.skipped} skipped",
                    )
                    Text(
                        "Imported words show the badge “${state.source}”." +
                            if (state.fromAnki) " They already have cards in Anki, so Allreads makes none for them." else "",
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                    Button(onClick = onClose, modifier = Modifier.fillMaxWidth()) { Text(stringResource(R.string.done)) }
                }
            }
        }
    }
}

/** A deck from AnkiDroid on this phone, or a file. */
@Composable
private fun ImportSourceSection(
    state: AnkiImportState,
    onDeck: (io.github.chflick.allreads.anki.AnkiDeck) -> Unit,
    onFile: () -> Unit,
) {
    Text(
        stringResource(R.string.words_you_study_in_anki),
        style = MaterialTheme.typography.bodyMedium,
    )
    when (state.access) {
        null -> Unit
        AnkiAccess.Connected -> SettingsDropdown(
            label = "A deck in AnkiDroid",
            selected = state.source?.takeIf { it.startsWith("Anki · ") && state.fromAnki }?.removePrefix("Anki · ") ?: "",
            options = state.decks.map { it.name to it },
            onSelect = onDeck,
        )
        else -> Text(
            stringResource(R.string.to_read_decks_from_ankidroid),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
    }
    OutlinedButton(onClick = onFile, modifier = Modifier.fillMaxWidth()) {
        Text(stringResource(R.string.or_a_file_apkg_deck))
    }
}

/** One note type (or file): its word and meaning field, with three cleaned samples. */
@Composable
private fun GroupCard(
    group: ImportGroup,
    choice: FieldChoice,
    samples: List<Sample>,
    onWord: (Int?) -> Unit,
    onMeaning: (Int?) -> Unit,
) {
    Card(colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer)) {
        Column(Modifier.padding(12.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Text(stringResource(R.string.notes_2, group.name, "%,d".format(group.notes.size)), style = MaterialTheme.typography.titleSmall)
            val fields = group.fields.mapIndexed { i, name -> name to i }
            SettingsDropdown(
                label = "Word",
                selected = choice.word?.let { group.fields.getOrNull(it) } ?: "Skip these notes",
                options = listOf<Pair<String, Int?>>("Skip these notes" to null) + fields,
                onSelect = onWord,
            )
            if (choice.word != null) {
                SettingsDropdown(
                    label = "Meaning (optional)",
                    selected = choice.meaning?.let { group.fields.getOrNull(it) } ?: "None",
                    options = listOf<Pair<String, Int?>>("None" to null) + fields,
                    onSelect = onMeaning,
                )
                samples.forEach { sample ->
                    Text(
                        stringResource(R.string.text_7, sample.raw.take(40), sample.word ?: "(skipped)"),
                        style = MaterialTheme.typography.bodySmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        maxLines = 1,
                    )
                }
            }
        }
    }
}

/** One level for every word, or Anki's review progress. */
@OptIn(ExperimentalLayoutApi::class)
@Composable
private fun StatusSection(state: AnkiImportState, onFollow: (Boolean) -> Unit, onLevel: (WordStatus) -> Unit) {
    Text(stringResource(R.string.how_well_do_you_know), style = MaterialTheme.typography.titleSmall)
    if (state.hasProgress) {
        Row(
            Modifier.fillMaxWidth().selectable(selected = state.followReviews, onClick = { onFollow(true) }),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            RadioButton(selected = state.followReviews, onClick = { onFollow(true) })
            Column {
                Text(stringResource(R.string.follow_anki_s_review_progress))
                Text(
                    stringResource(R.string.new_cards_1_then_2),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        Row(
            Modifier.fillMaxWidth().selectable(selected = !state.followReviews, onClick = { onFollow(false) }),
            verticalAlignment = Alignment.CenterVertically,
        ) {
            RadioButton(selected = !state.followReviews, onClick = { onFollow(false) })
            Text(stringResource(R.string.one_level_for_all))
        }
    }
    if (!state.followReviews || !state.hasProgress) {
        FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            wordStatusChoices.filter { it != WordStatus.New && it != WordStatus.Ignored }.forEach { status ->
                StatusChip(status, selected = state.level == status, onClick = { onLevel(status) })
            }
        }
    }
    Text(
        stringResource(R.string.a_word_you_already_have),
        style = MaterialTheme.typography.bodySmall,
        color = MaterialTheme.colorScheme.onSurfaceVariant,
    )
}
