package io.github.chflick.allreads

import io.github.chflick.allreads.ui.words.progressSeries
import java.time.LocalDate
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.allreads_ffi.DayCount

class ProgressTest {
    private val today = LocalDate.parse("2026-11-26")

    @Test
    fun gapsCarryTheDayBeforeForward() {
        val series = progressSeries(
            listOf(DayCount("2026-11-22", 10u, 5u), DayCount("2026-11-24", 14u, 3u)),
            today,
        )
        assertEquals(5, series.days.size)
        assertEquals(listOf(10, 10, 14, 14, 14), series.known)
        assertEquals(listOf(5, 5, 3, 3, 3), series.learning)
        assertEquals(14, series.knownNow)
        assertEquals(4, series.knownThisWeek)
    }

    @Test
    fun onlyTheLastDaysAreKept() {
        val series = progressSeries(
            listOf(DayCount("2026-01-01", 1u, 0u), DayCount("2026-11-20", 20u, 0u)),
            today,
            maxDays = 10,
        )
        assertEquals(10, series.days.size)
        assertEquals(LocalDate.parse("2026-11-17"), series.days.first())
        // The count before the window is carried into it.
        assertEquals(listOf(1, 1, 1, 20, 20, 20, 20, 20, 20, 20), series.known)
        assertEquals(19, series.knownThisWeek)
    }

    @Test
    fun noHistoryIsEmpty() {
        assertTrue(progressSeries(emptyList(), today).days.isEmpty())
    }
}
