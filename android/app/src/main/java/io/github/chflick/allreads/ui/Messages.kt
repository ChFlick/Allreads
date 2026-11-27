package io.github.chflick.allreads.ui

import io.github.chflick.allreads.R
import io.github.chflick.allreads.ui.AppText
import uniffi.allreads_ffi.AllreadsException

/** A short, readable message for a snackbar. */
fun describe(error: Throwable): String = when (error) {
    is AllreadsException.TranslationKeyRejected -> AppText.get(R.string.deepl_rejected_the_api_key)
    is AllreadsException.TranslationQuotaExceeded -> AppText.get(R.string.your_deepl_quota_is_used)
    is AllreadsException.TranslationRateLimited -> AppText.get(R.string.deepl_is_busy_try_again)
    is AllreadsException.TranslationUnavailable -> AppText.get(R.string.translation_needs_a_network_connection)
    is AllreadsException.NotFound -> AppText.get(R.string.file_not_found, error.path)
    is AllreadsException.UnsupportedFile -> AppText.get(R.string.only_epub_and_txt_books_2)
    is AllreadsException.DictionaryDamaged -> AppText.get(R.string.the_download_was_damaged_try)
    is AllreadsException.DictionaryInvalid -> AppText.get(R.string.this_dictionary_file_cannot_be)
    is AllreadsException.CatalogInvalid -> AppText.get(R.string.this_book_list_cannot_be_used)
    is java.io.IOException -> AppText.get(R.string.the_download_failed_check_the)
    else -> error.message ?: error::class.simpleName ?: AppText.get(R.string.something_went_wrong)
}

/** Shown when a translation is asked for before a DeepL key is saved. */
val NO_DEEPL_KEY: String get() = AppText.get(R.string.add_your_deepl_api_key)
