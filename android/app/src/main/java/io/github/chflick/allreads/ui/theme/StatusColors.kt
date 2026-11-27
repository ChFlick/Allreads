package io.github.chflick.allreads.ui.theme

import io.github.chflick.allreads.R
import io.github.chflick.allreads.ui.AppText
import androidx.compose.ui.graphics.Color
import uniffi.allreads_ffi.WordStatus

/**
 * The background a word gets. Words being learned are amber, strongest at
 * level 1 and fading towards level 5, so a page shows at a glance how much is
 * left to learn. Dark mode uses deep tones so light text stays readable.
 * Known and ignored words stay plain. In the reader, new words get no fill,
 * only a dotted underline; the blue is for chips and the Words tab.
 */
fun WordStatus.highlight(dark: Boolean): Color? = when (this) {
    WordStatus.New -> if (dark) Color(0xFF27496D) else Color(0xFFD6E3FF)
    is WordStatus.Learning -> when (level.toInt()) {
        1 -> if (dark) Color(0xFF8A5A10) else Color(0xFFFFB74D)
        2 -> if (dark) Color(0xFF725018) else Color(0xFFFFC978)
        3 -> if (dark) Color(0xFF5C451D) else Color(0xFFFFD9A1)
        4 -> if (dark) Color(0xFF483B20) else Color(0xFFFFE7C6)
        else -> if (dark) Color(0xFF383124) else Color(0xFFFFF1DE)
    }
    WordStatus.WellKnown, WordStatus.Ignored -> null
}

/** A longer label for lists, e.g. "Level 2". */
fun WordStatus.longLabel(): String = when (this) {
    WordStatus.New -> AppText.get(R.string.status_new)
    is WordStatus.Learning -> AppText.get(R.string.status_level, level.toString())
    WordStatus.WellKnown -> AppText.get(R.string.known_2)
    WordStatus.Ignored -> AppText.get(R.string.ignored)
}

/** A short label for status chips. */
fun WordStatus.label(): String = when (this) {
    WordStatus.New -> AppText.get(R.string.status_new)
    is WordStatus.Learning -> level.toString()
    WordStatus.WellKnown -> AppText.get(R.string.known_2)
    WordStatus.Ignored -> AppText.get(R.string.status_ignore)
}

/** Every status the word sheet offers, in display order. */
val wordStatusChoices: List<WordStatus> =
    listOf(WordStatus.New) + (1..5).map { WordStatus.Learning(it.toUByte()) } +
        listOf(WordStatus.WellKnown, WordStatus.Ignored)
