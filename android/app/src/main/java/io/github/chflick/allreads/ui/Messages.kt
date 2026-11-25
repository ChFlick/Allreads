package io.github.chflick.allreads.ui

import uniffi.allreads_ffi.AllreadsException

/** A short, readable message for a snackbar. */
fun describe(error: Throwable): String = when (error) {
    is AllreadsException.TranslationKeyRejected -> "DeepL rejected the API key. Check it in Settings."
    is AllreadsException.TranslationQuotaExceeded -> "Your DeepL quota is used up for this month."
    is AllreadsException.TranslationRateLimited -> "DeepL is busy. Try again in a moment."
    is AllreadsException.TranslationUnavailable -> "Translation needs a network connection."
    is AllreadsException.NotFound -> "File not found: ${error.path}"
    is AllreadsException.UnsupportedFile -> "Only EPUB and TXT books can be imported."
    is AllreadsException.DictionaryDamaged -> "The download was damaged. Try again."
    is AllreadsException.DictionaryInvalid -> "This dictionary file cannot be used."
    is java.io.IOException -> "The download failed. Check the connection and try again."
    else -> error.message ?: error::class.simpleName ?: "Something went wrong"
}

/** Shown when a translation is asked for before a DeepL key is saved. */
const val NO_DEEPL_KEY = "Add your DeepL API key in Settings to translate."
