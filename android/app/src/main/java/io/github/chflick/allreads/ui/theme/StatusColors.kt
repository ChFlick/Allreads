package io.github.chflick.allreads.ui.theme

import androidx.compose.ui.graphics.Color
import uniffi.allreads_ffi.WordStatus

/**
 * The background a word gets in the reader. New words are blue, words being
 * learned fade from orange (level 1) to pale green (level 5), and known or
 * ignored words are left plain. Colour is not the only cue: see the reader's
 * underline for new words.
 */
fun WordStatus.highlight(): Color? = when (this) {
    WordStatus.New -> Color(0xFFBBDEFB)
    is WordStatus.Learning -> when (level.toInt()) {
        1 -> Color(0xFFFFCC80)
        2 -> Color(0xFFFFE082)
        3 -> Color(0xFFFFF59D)
        4 -> Color(0xFFF0F4C3)
        else -> Color(0xFFDCEDC8)
    }
    WordStatus.WellKnown, WordStatus.Ignored -> null
}

/** A short label for the status chips in the word sheet. */
fun WordStatus.label(): String = when (this) {
    WordStatus.New -> "New"
    is WordStatus.Learning -> level.toString()
    WordStatus.WellKnown -> "Known"
    WordStatus.Ignored -> "Ignore"
}

/** Every status the word sheet offers, in display order. */
val wordStatusChoices: List<WordStatus> =
    listOf(WordStatus.New) + (1..5).map { WordStatus.Learning(it.toUByte()) } +
        listOf(WordStatus.WellKnown, WordStatus.Ignored)
