package io.github.chflick.allreads.ui

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.ArrowDropDown
import androidx.compose.material3.AssistChip
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.Icon
import androidx.compose.material3.RadioButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp

/**
 * "↕ Recently read ▾": shows the current order and opens a menu to change it.
 * `description` is what TalkBack says, e.g. "Sort books, now by title".
 */
@Composable
fun <T> SortChip(current: T, choices: List<T>, label: (T) -> String, description: String, onSort: (T) -> Unit) {
    var open by remember { mutableStateOf(false) }
    Box {
        AssistChip(
            onClick = { open = true },
            label = { Text(label(current)) },
            leadingIcon = { Icon(SortIcon, contentDescription = null, modifier = Modifier.size(18.dp)) },
            trailingIcon = { Icon(Icons.Default.ArrowDropDown, contentDescription = null, modifier = Modifier.size(18.dp)) },
            modifier = Modifier.semantics { contentDescription = description },
        )
        DropdownMenu(expanded = open, onDismissRequest = { open = false }) {
            choices.forEach { choice ->
                DropdownMenuItem(
                    text = { Text(label(choice)) },
                    leadingIcon = { RadioButton(selected = choice == current, onClick = null) },
                    onClick = {
                        open = false
                        onSort(choice)
                    },
                )
            }
        }
    }
}
