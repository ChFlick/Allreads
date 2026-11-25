package io.github.chflick.allreads.ui

import java.util.Locale

/** A size as Settings and the word drawer show it, e.g. "10.8 MB". */
/** A language's name in the phone's language, e.g. "French" for `fr`. */
fun languageName(code: String): String =
    Locale.forLanguageTag(code).displayLanguage.takeIf { it.isNotBlank() && it != code }
        ?.replaceFirstChar { it.titlecase() } ?: code.uppercase()

fun megabytes(bytes: Long): String = "%.1f MB".format(Locale.ENGLISH, bytes / 1_000_000.0)
