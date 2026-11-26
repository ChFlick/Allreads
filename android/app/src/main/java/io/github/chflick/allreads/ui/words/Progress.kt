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
import java.time.LocalDate
import uniffi.allreads_ffi.DayCount

/** Known and learning words on each day, gaps filled with the day before. */
data class ProgressSeries(val days: List<LocalDate>, val known: List<Int>, val learning: List<Int>) {
    val knownNow: Int get() = known.lastOrNull() ?: 0
    val learningNow: Int get() = learning.lastOrNull() ?: 0

    /** Known words gained in the last 7 days (0 if fewer are recorded). */
    val knownThisWeek: Int
        get() {
            if (known.isEmpty()) return 0
            val weekAgo = (known.size - 8).coerceAtLeast(0)
            return known.last() - known[weekAgo]
        }
}

/**
 * Turns the stored counts (one per day with a change) into one value per day
 * up to `today`, at most the last `maxDays` days.
 */
fun progressSeries(history: List<DayCount>, today: LocalDate, maxDays: Int = 90): ProgressSeries {
    val byDay = history.mapNotNull { count -> runCatching { LocalDate.parse(count.day) }.getOrNull()?.let { it to count } }
        .sortedBy { it.first }
    if (byDay.isEmpty()) return ProgressSeries(emptyList(), emptyList(), emptyList())
    val first = maxOf(byDay.first().first, today.minusDays((maxDays - 1).toLong()))
    val days = generateSequence(first) { it.plusDays(1) }.takeWhile { !it.isAfter(today) }.toList()
    val known = mutableListOf<Int>()
    val learning = mutableListOf<Int>()
    var index = 0
    var current: DayCount? = null
    for (day in days) {
        // Move to the last count on or before this day.
        while (index < byDay.size && !byDay[index].first.isAfter(day)) {
            current = byDay[index].second
            index++
        }
        known += current?.known?.toInt() ?: 0
        learning += current?.learning?.toInt() ?: 0
    }
    return ProgressSeries(days, known, learning)
}

/** "1,204 known words · +56 this week", the learning count, and a chart of both. */
@Composable
fun ProgressCard(series: ProgressSeries, modifier: Modifier = Modifier) {
    if (series.days.isEmpty()) return
    val knownColour = MaterialTheme.colorScheme.primary
    val learningColour = Color(0xFFE0A030)
    Card(
        modifier.fillMaxWidth(),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainer),
    ) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Row(verticalAlignment = Alignment.Bottom, horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("%,d".format(series.knownNow), style = MaterialTheme.typography.headlineMedium)
                Text("known words", style = MaterialTheme.typography.bodyMedium, modifier = Modifier.padding(bottom = 4.dp))
                if (series.knownThisWeek > 0) {
                    Text(
                        "+%,d this week".format(series.knownThisWeek),
                        style = MaterialTheme.typography.labelLarge,
                        color = MaterialTheme.colorScheme.primary,
                        modifier = Modifier.padding(bottom = 5.dp),
                    )
                }
            }
            if (series.days.size > 1) {
                ProgressChart(series, knownColour, learningColour)
                Row(horizontalArrangement = Arrangement.spacedBy(16.dp)) {
                    Legend(knownColour, "Known")
                    Legend(learningColour, "Learning (%,d)".format(series.learningNow))
                }
            } else {
                Text(
                    "%,d words being learned. The chart starts tomorrow.".format(series.learningNow),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    }
}

@Composable
private fun Legend(colour: Color, label: String) {
    Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(6.dp)) {
        Box(Modifier.size(8.dp).background(colour, CircleShape))
        Text(label, style = MaterialTheme.typography.labelMedium)
    }
}

/** Two lines over the days, both against the larger of the two counts. */
@Composable
private fun ProgressChart(series: ProgressSeries, knownColour: Color, learningColour: Color) {
    val grid = MaterialTheme.colorScheme.outlineVariant
    Canvas(Modifier.fillMaxWidth().height(96.dp)) {
        val top = maxOf(series.known.maxOrNull() ?: 0, series.learning.maxOrNull() ?: 0, 1)
        val step = size.width / (series.days.size - 1)
        fun y(value: Int) = size.height - value.toFloat() / top * size.height
        drawLine(grid, Offset(0f, size.height), Offset(size.width, size.height), strokeWidth = 1.dp.toPx())
        for ((values, colour) in listOf(series.learning to learningColour, series.known to knownColour)) {
            val path = Path()
            values.forEachIndexed { i, value ->
                if (i == 0) path.moveTo(0f, y(value)) else path.lineTo(i * step, y(value))
            }
            drawPath(path, colour, style = Stroke(width = 2.5.dp.toPx()))
        }
    }
}
