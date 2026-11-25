package io.github.chflick.allreads.ui.reader

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
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.rememberModalBottomSheetState
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import android.widget.Toast
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import io.github.chflick.allreads.ui.theme.label
import io.github.chflick.allreads.ui.theme.wordStatusChoices
import uniffi.allreads_ffi.WordStatus

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
    onSave: (WordStatus, String, String) -> Unit,
    onDismiss: () -> Unit,
) {
    // A first tap on a new word suggests level 1, the usual choice.
    var status by rememberSaveable(word.text) {
        mutableStateOf(if (word.status == WordStatus.New) WordStatus.Learning(1u) else word.status)
    }
    var translation by rememberSaveable(word.text) { mutableStateOf(word.saved?.translation ?: "") }
    var notes by rememberSaveable(word.text) { mutableStateOf(word.saved?.notes ?: "") }
    // Fill an empty translation field with DeepL's answer when it arrives.
    LaunchedEffect(word.machineTranslation) {
        val machine = word.machineTranslation
        if (machine != null && translation.isBlank()) translation = machine
    }
    val context = LocalContext.current

    ModalBottomSheet(onDismissRequest = onDismiss, sheetState = rememberModalBottomSheetState()) {
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
                    modifier = Modifier.weight(1f),
                )
                Button(onClick = { onSave(status, translation, notes) }) {
                    Text(if (status == WordStatus.New) "Forget" else "Save")
                }
            }
            when {
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
            OutlinedTextField(
                value = translation,
                onValueChange = { translation = it },
                label = { Text("Translation") },
                singleLine = true,
                modifier = Modifier.fillMaxWidth(),
            )
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                wordStatusChoices.forEach { choice ->
                    FilterChip(
                        selected = status == choice,
                        onClick = { status = choice },
                        label = { Text(choice.label()) },
                    )
                }
            }

            // --- Pull the drawer up for more context ---
            HorizontalDivider(Modifier.padding(top = 8.dp))
            Text("In this sentence", style = MaterialTheme.typography.labelLarge)
            Text(word.sentence, style = MaterialTheme.typography.bodyLarge)
            OutlinedTextField(
                value = notes,
                onValueChange = { notes = it },
                label = { Text("Notes") },
                minLines = 2,
                modifier = Modifier.fillMaxWidth(),
            )
            word.saved?.lemma?.let { Text("Dictionary form: $it", style = MaterialTheme.typography.bodyMedium) }
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
}
