package io.github.chflick.allreads.ui.theme

import androidx.compose.ui.graphics.Color
import uniffi.allreads_ffi.WordStatus

/**
 * The background a word gets. Words being learned fade from orange (level 1)
 * to green (level 5); dark mode uses deep tones so light text stays readable.
 * Known and ignored words stay plain. In the reader, new words get no fill,
 * only a faint underline, so a fresh book is not a wall of colour; the Words
 * tab uses the blue for its status dots.
 */
fun WordStatus.highlight(dark: Boolean): Color? = when (this) {
    WordStatus.New -> if (dark) Color(0xFF27496D) else Color(0xFFBBDEFB)
    is WordStatus.Learning -> when (level.toInt()) {
        1 -> if (dark) Color(0xFF7A4512) else Color(0xFFFFCC80)
        2 -> if (dark) Color(0xFF6E5514) else Color(0xFFFFE082)
        3 -> if (dark) Color(0xFF5F5A1C) else Color(0xFFFFF59D)
        4 -> if (dark) Color(0xFF4B5523) else Color(0xFFF0F4C3)
        else -> if (dark) Color(0xFF3B4F2C) else Color(0xFFDCEDC8)
    }
    WordStatus.WellKnown, WordStatus.Ignored -> null
}

/** A short label for status chips. */
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
