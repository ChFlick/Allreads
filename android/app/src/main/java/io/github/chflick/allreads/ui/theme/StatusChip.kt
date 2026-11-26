package io.github.chflick.allreads.ui.theme

import androidx.compose.foundation.background
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
            { Icon(Icons.Default.Check, contentDescription = null, modifier = Modifier.padding(0.dp)) }
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
fun StatusPill(status: WordStatus) {
    val fill = status.highlight(LocalDarkTheme.current)
    val shape = MaterialTheme.shapes.small
    val modifier = if (fill != null) {
        Modifier.background(fill, shape)
    } else {
        Modifier.border(1.dp, MaterialTheme.colorScheme.outlineVariant, shape)
    }
    Text(
        status.longLabel(),
        style = MaterialTheme.typography.labelMedium,
        color = MaterialTheme.colorScheme.onSurface,
        modifier = modifier.padding(horizontal = 8.dp, vertical = 3.dp),
    )
}
