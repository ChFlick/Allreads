package io.github.chflick.allreads.ui

import java.util.Locale

/** A size as Settings and the word drawer show it, e.g. "10.8 MB". */
fun megabytes(bytes: Long): String = "%.1f MB".format(Locale.ENGLISH, bytes / 1_000_000.0)
