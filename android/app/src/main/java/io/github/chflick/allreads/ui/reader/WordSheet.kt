package io.github.chflick.allreads.ui.reader

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.ModalBottomSheet
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import io.github.chflick.allreads.ui.theme.label
import io.github.chflick.allreads.ui.theme.wordStatusChoices
import uniffi.allreads_ffi.WordStatus

/** Step 13: the word, its sentence, a status and a translation. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun WordSheet(
    word: SelectedWord,
    onSave: (WordStatus, String) -> Unit,
    onDismiss: () -> Unit,
) {
    // A first tap on a new word suggests level 1, the usual choice.
    var status by rememberSaveable(word.text) {
        mutableStateOf(if (word.status == WordStatus.New) WordStatus.Learning(1u) else word.status)
    }
    var translation by rememberSaveable(word.text) { mutableStateOf(word.saved?.translation ?: "") }

    ModalBottomSheet(onDismissRequest = onDismiss) {
        Column(
            Modifier.fillMaxWidth().padding(horizontal = 20.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Text(word.saved?.displayText ?: word.text, style = MaterialTheme.typography.headlineSmall)
            Text(word.sentence, style = MaterialTheme.typography.bodyMedium)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                wordStatusChoices.forEach { choice ->
                    FilterChip(
                        selected = status == choice,
                        onClick = { status = choice },
                        label = { Text(choice.label()) },
                    )
                }
            }
            OutlinedTextField(
                value = translation,
                onValueChange = { translation = it },
                label = { Text("Translation") },
                modifier = Modifier.fillMaxWidth(),
            )
            Button(
                onClick = { onSave(status, translation) },
                modifier = Modifier.fillMaxWidth().padding(bottom = 16.dp),
            ) { Text(if (status == WordStatus.New) "Mark as new" else "Save") }
        }
    }
}
