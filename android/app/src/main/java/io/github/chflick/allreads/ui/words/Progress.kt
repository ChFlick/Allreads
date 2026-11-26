package io.github.chflick.allreads.ui.words

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.background
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import io.github.chflick.allreads.ui.DotLabel
import uniffi.allreads_ffi.DayCount
import uniffi.allreads_ffi.WordProgress

/** "1,204 known words · +56 this week", the learning count, and a chart of both. */
@Composable
fun ProgressCard(progress: WordProgress, modifier: Modifier = Modifier) {
    if (progress.days.isEmpty()) return
    val now = progress.days.last()
    val knownColour = MaterialTheme.colorScheme.primary
    val learningColour = Color(0xFFE0A030)
    Card(
        modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("%,d".format(now.known.toInt()), style = MaterialTheme.typography.headlineMedium)
                Text("known words", style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(bottom = 4.dp))
                if (progress.knownThisWeek > 0u) {
                    Text(
                        "+%,d this week".format(progress.knownThisWeek.toInt()),
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(bottom = 5.dp),
                    )
                }
            }
            if (progress.days.size > 1) {
                ProgressChart(progress.days, knownColour, learningColour)
                Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    DotLabel(knownColour, "Known")
                    DotLabel(learningColour, "Learning (%,d)".format(now.learning.toInt()))
                }
            } else {
                Text(
                    "%,d words being learned. The chart starts tomorrow.".format(now.learning.toInt()),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

/** Two lines over the days, both against the larger of the two counts. */
@Composable
private fun ProgressChart(days: List<DayCount>, knownColour: Color, learningColour: Color) {
    val grid = MaterialTheme.colorScheme.outlineVariant
    Canvas(Modifier.fillMaxWidth().height(96.dp)) {
        val known = days.map { it.known.toInt() }
        val learning = days.map { it.learning.toInt() }
        val top = maxOf(known.maxOrNull() ?: 0, learning.maxOrNull() ?: 0, 1)
        val step = size.width / (days.size - 1)
        fun y(value: Int) = size.height - value.toFloat() / top * size.height
        drawLine(grid, Offset(0f, size.height), Offset(size.width, size.height), strokeWidth = 1.dp.toPx())
        for ((values, colour) in listOf(learning to learningColour, known to knownColour)) {
            val path = Path()
            values.forEachIndexed { i, value ->
                if (i == 0) path.moveTo(0f, y(value)) else path.lineTo(i * step, y(value))
            }
            drawPath(path, colour, style = Stroke(width = 2.5.dp.toPx()))
        }
    }
}
