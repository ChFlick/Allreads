package io.github.chflick.allreads.ui.theme

import androidx.compose.foundation.BorderStroke
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxHeight
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Surface
import androidx.compose.ui.Alignment
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.foundation.border
import androidx.compose.foundation.layout.padding
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Check
import androidx.compose.material3.FilterChip
import androidx.compose.material3.FilterChipDefaults
import androidx.compose.material3.Icon
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import uniffi.allreads_ffi.WordStatus

/** A status chip filled with the status's reader colour, so the chips teach the colours. */
@Composable
fun StatusChip(status: WordStatus, selected: Boolean, onClick: () -> Unit) {
    val fill = status.highlight(LocalDarkTheme.current) ?: MaterialTheme.colorScheme.surface
    FilterChip(
        selected = selected,
        onClick = onClick,
        label = { Text(status.label()) },
        leadingIcon = if (selected) {
            { Icon(Icons.Default.Check, contentDescription = null) }
        } else {
            null
        },
        colors = FilterChipDefaults.filterChipColors(
            containerColor = fill,
            selectedContainerColor = fill,
            labelColor = MaterialTheme.colorScheme.onSurface,
            selectedLabelColor = MaterialTheme.colorScheme.onSurface,
            selectedLeadingIconColor = MaterialTheme.colorScheme.onSurface,
        ),
        border = FilterChipDefaults.filterChipBorder(
            enabled = true,
            selected = selected,
            borderColor = MaterialTheme.colorScheme.outlineVariant,
            selectedBorderColor = MaterialTheme.colorScheme.onSurface,
            selectedBorderWidth = 1.5.dp,
        ),
    )
}

/** A small pill in the status colour with "Level 2", "Known" and so on. */
@Composable
fun StatusPill(status: WordStatus, short: Boolean = false) {
    val fill = status.highlight(LocalDarkTheme.current)
    val shape = MaterialTheme.shapes.small
    val modifier = if (fill != null) {
        Modifier.background(fill, shape)
    } else {
        Modifier.border(1.dp, MaterialTheme.colorScheme.outlineVariant, shape)
    }
    Text(
        // Short pills show a learning level as its number; the others keep their name.
        if (short && status is WordStatus.Learning) status.label() else status.longLabel(),
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurface,
        modifier = modifier.padding(horizontal = 8.dp, vertical = 3.dp),
    )
}

/**
 * New · 1–5 · Known · Ignore as one row of cells in their level colours, so
 * the half-open word drawer shows all levels at once. The selected one has a
 * dark outline; Known and Ignore are icons, with spoken descriptions.
 */
@Composable
fun LevelBar(selected: WordStatus?, onSelect: (WordStatus) -> Unit, modifier: Modifier = Modifier) {
    val dark = LocalDarkTheme.current
    Row(modifier.fillMaxWidth().height(40.dp), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
        wordStatusChoices.forEach { status ->
            val weight = when (status) {
                WordStatus.New -> 1.4f
                WordStatus.WellKnown -> 1.2f
                else -> 1f
            }
            val isSelected = status == selected
            Surface(
                onClick = { onSelect(status) },
                shape = MaterialTheme.shapes.small,
                color = status.highlight(dark) ?: MaterialTheme.colorScheme.surface,
                border = if (isSelected) {
                    BorderStroke(2.dp, MaterialTheme.colorScheme.onSurface)
                } else {
                    BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant)
                },
                modifier = Modifier.weight(weight).fillMaxHeight().semantics {
                    contentDescription = status.longLabel()
                    // TalkBack says "selected" itself, in the phone's language.
                    this.selected = isSelected
                },
            ) {
                Box(contentAlignment = Alignment.Center) {
                    when (status) {
                        WordStatus.WellKnown -> Icon(Icons.Default.Check, contentDescription = null, modifier = Modifier.size(18.dp))
                        WordStatus.Ignored -> Text("⊘", style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.onSurfaceVariant)
                        else -> Text(status.label(), style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.onSurface)
                    }
                }
            }
        }
    }
}
