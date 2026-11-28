package io.github.chflick.allreads.ui.vocab

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.selection.selectable
import androidx.compose.foundation.verticalScroll
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Close
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.chflick.allreads.R
import io.github.chflick.allreads.ui.languageName
import io.github.chflick.allreads.ui.megabytes
import io.github.chflick.allreads.ui.theme.Literata
import kotlin.math.roundToInt
import uniffi.allreads_ffi.VocabChoice

/** The vocabulary check, full screen. */
@Composable
fun VocabCheckScreen(viewModel: VocabCheckViewModel, onClose: () -> Unit) {
    val state by viewModel.state.collectAsStateWithLifecycle()
    VocabCheckContent(
        state = state,
        language = languageName(viewModel.languageCode),
        onAnswer = viewModel::answer,
        onBack = viewModel::back,
        onDownload = viewModel::downloadDictionary,
        onChoose = viewModel::choose,
        onApply = viewModel::apply,
        onRetake = viewModel::start,
        onClose = onClose,
    )
}

/** The check without its view model, for screenshots. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun VocabCheckContent(
    state: VocabCheckState,
    language: String,
    onAnswer: (Boolean) -> Unit = {},
    onBack: () -> Unit = {},
    onDownload: () -> Unit = {},
    onChoose: (Int) -> Unit = {},
    onApply: () -> Unit = {},
    onRetake: () -> Unit = {},
    onClose: () -> Unit = {},
) {
    BackHandler(onBack = onClose)
    Scaffold(
        topBar = {
            Column {
                TopAppBar(
                    title = { Text(stringResource(R.string.vocabulary_check)) },
                    navigationIcon = {
                        IconButton(onClick = onClose) {
                            Icon(Icons.Default.Close, contentDescription = stringResource(R.string.close))
                        }
                    },
                )
                if (state is VocabCheckState.Asking) {
                    LinearProgressIndicator(
                        progress = { state.progress },
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 16.dp).height(4.dp),
                        drawStopIndicator = {},
                    )
                }
            }
        },
    ) { padding ->
        Box(Modifier.fillMaxSize().padding(padding), contentAlignment = Alignment.TopCenter) {
            // A comfortable column on tablets too.
            Box(Modifier.widthIn(max = 560.dp).fillMaxSize()) {
                when (state) {
                    VocabCheckState.Starting -> Box(Modifier.fillMaxSize(), contentAlignment = Alignment.Center) {
                        CircularProgressIndicator()
                    }
                    is VocabCheckState.NeedsDictionary -> Column(
                        Modifier.fillMaxSize().padding(24.dp),
                        verticalArrangement = Arrangement.spacedBy(16.dp),
                    ) {
                        Text(stringResource(R.string.vocab_needs_dictionary_title, language), style = MaterialTheme.typography.headlineSmall)
                        Text(
                            stringResource(R.string.vocab_needs_dictionary_text, language, megabytes(state.downloadBytes)),
                            style = MaterialTheme.typography.bodyMedium,
                        )
                        Button(onClick = onDownload, modifier = Modifier.fillMaxWidth().height(52.dp)) {
                            Text(stringResource(R.string.download_and_start))
                        }
                        TextButton(onClick = onClose, modifier = Modifier.align(Alignment.End)) { Text(stringResource(R.string.not_now)) }
                    }
                    is VocabCheckState.Downloading -> Column(
                        Modifier.fillMaxSize().padding(24.dp),
                        verticalArrangement = Arrangement.spacedBy(16.dp),
                    ) {
                        Text(stringResource(R.string.downloading_the_dictionary, language), style = MaterialTheme.typography.titleMedium)
                        LinearProgressIndicator(progress = { state.progress.coerceIn(0f, 1f) }, modifier = Modifier.fillMaxWidth())
                    }
                    is VocabCheckState.Done -> Column(
                        Modifier.fillMaxSize().padding(24.dp),
                        verticalArrangement = Arrangement.spacedBy(16.dp),
                    ) {
                        Text(
                            pluralStringResource(R.plurals.words_marked_known, state.words, "%,d".format(state.words)),
                            style = MaterialTheme.typography.headlineSmall,
                        )
                        Text(
                            pluralStringResource(R.plurals.vocab_done_explained, state.forms, "%,d".format(state.forms)),
                            style = MaterialTheme.typography.bodyMedium,
                        )
                        Button(onClick = onClose, modifier = Modifier.fillMaxWidth().height(52.dp)) {
                            Text(stringResource(R.string.done))
                        }
                    }
                    is VocabCheckState.Asking -> Question(state.word, language, state.canGoBack, onAnswer, onBack)
                    is VocabCheckState.Result -> ResultView(state, language, onChoose, onApply, onRetake, onClose)
                    is VocabCheckState.Failed -> Column(
                        Modifier.fillMaxSize().padding(24.dp),
                        verticalArrangement = Arrangement.spacedBy(16.dp),
                    ) {
                        Text(state.message, style = MaterialTheme.typography.bodyLarge)
                        OutlinedButton(onClick = onClose) { Text(stringResource(R.string.close)) }
                    }
                }
            }
        }
    }
}

/** One word, large, with No and Yes. */
@Composable
private fun Question(word: String, language: String, canGoBack: Boolean, onAnswer: (Boolean) -> Unit, onBack: () -> Unit) {
    Column(
        Modifier.fillMaxSize().padding(24.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        Text(
            stringResource(R.string.do_you_know_this_word, language),
            style = MaterialTheme.typography.titleMedium,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
        )
        Spacer(Modifier.weight(1f))
        Text(word, fontFamily = Literata, fontSize = 40.sp, textAlign = TextAlign.Center)
        Spacer(Modifier.weight(1f))
        Text(
            stringResource(R.string.only_yes_if_you_know_the_meaning),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            textAlign = TextAlign.Center,
            modifier = Modifier.padding(bottom = 16.dp),
        )
        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            FilledTonalButton(onClick = { onAnswer(false) }, modifier = Modifier.weight(1f).height(56.dp)) {
                Text(stringResource(R.string.no))
            }
            Button(onClick = { onAnswer(true) }, modifier = Modifier.weight(1f).height(56.dp)) {
                Text(stringResource(R.string.yes))
            }
        }
        // Takes back a mis-tap; only within the words asked since the last band was sent.
        TextButton(onClick = onBack, enabled = canGoBack, modifier = Modifier.padding(top = 4.dp)) {
            Text(stringResource(R.string.back_to_previous_word))
        }
    }
}

/** What the check found, the choices with their accuracy, and Mark. */
@Composable
private fun ResultView(
    state: VocabCheckState.Result,
    language: String,
    onChoose: (Int) -> Unit,
    onApply: () -> Unit,
    onRetake: () -> Unit,
    onClose: () -> Unit,
) {
    val result = state.result
    Column(
        Modifier.fillMaxSize().verticalScroll(rememberScrollState()).padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp),
    ) {
        if (result.reliable) {
            Text(
                pluralStringResource(
                    R.plurals.you_know_about_words,
                    result.estimatedKnown.toInt(),
                    "%,d".format(result.estimatedKnown.toInt()),
                    language,
                ),
                style = MaterialTheme.typography.headlineSmall,
            )
            Text(stringResource(R.string.vocab_result_explained), style = MaterialTheme.typography.bodyMedium)
        } else {
            Text(stringResource(R.string.vocab_result_unsure), style = MaterialTheme.typography.headlineSmall)
            Text(stringResource(R.string.vocab_result_unsure_explained), style = MaterialTheme.typography.bodyMedium)
        }
        result.choices.forEachIndexed { index, choice ->
            ChoiceCard(choice, recommended = index == 0 && result.choices.size > 1, selected = index == state.chosen, onSelect = { onChoose(index) })
        }
        Text(
            stringResource(R.string.vocab_result_fine_print),
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
        )
        if (result.choices.isEmpty()) {
            // Answers too unsure to mark anything: a retake is the way on.
            Button(onClick = onRetake, modifier = Modifier.fillMaxWidth().height(52.dp)) {
                Text(stringResource(R.string.retake_the_check))
            }
            TextButton(onClick = onClose, modifier = Modifier.align(Alignment.End)) { Text(stringResource(R.string.not_now)) }
        } else {
            val chosenWords = result.choices.getOrNull(state.chosen)?.words?.toInt() ?: 0
            Button(onClick = onApply, enabled = chosenWords > 0 && !state.applying, modifier = Modifier.fillMaxWidth().height(52.dp)) {
                Text(pluralStringResource(R.plurals.mark_words_known, chosenWords, "%,d".format(chosenWords)))
            }
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                TextButton(onClick = onRetake) { Text(stringResource(R.string.retake_the_check)) }
                TextButton(onClick = onClose) { Text(stringResource(R.string.not_now)) }
            }
        }
    }
}

/** "Mark 820 words · about 98 % right", selectable. */
@Composable
private fun ChoiceCard(choice: VocabChoice, recommended: Boolean, selected: Boolean, onSelect: () -> Unit) {
    val words = choice.words.toInt()
    Surface(
        shape = MaterialTheme.shapes.medium,
        border = BorderStroke(
            if (selected) 2.dp else 1.dp,
            if (selected) MaterialTheme.colorScheme.primary else MaterialTheme.colorScheme.outlineVariant,
        ),
        modifier = Modifier.fillMaxWidth().selectable(selected = selected, role = Role.RadioButton, onClick = onSelect),
    ) {
        Row(Modifier.padding(16.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            RadioButton(selected = selected, onClick = null)
            Column(Modifier.weight(1f), verticalArrangement = Arrangement.spacedBy(2.dp)) {
                Text(
                    pluralStringResource(R.plurals.mark_n_words, words, "%,d".format(words)),
                    style = MaterialTheme.typography.titleMedium,
                )
                Text(
                    stringResource(R.string.about_percent_right, (choice.expectedAccuracy * 100).roundToInt()),
                    style = MaterialTheme.typography.bodyMedium,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
                if (recommended) {
                    Text(
                        stringResource(R.string.recommended),
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.primary,
                    )
                }
            }
        }
    }
}
