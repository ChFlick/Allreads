package io.github.chflick.allreads.ui.settings

import io.github.chflick.allreads.ui.AppText
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import io.github.chflick.allreads.R
import android.content.Context
import android.net.ConnectivityManager
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.chflick.allreads.data.Catalogs
import io.github.chflick.allreads.data.Dictionaries
import io.github.chflick.allreads.data.Services
import io.github.chflick.allreads.data.DownloadState
import io.github.chflick.allreads.ui.describe
import io.github.chflick.allreads.ui.megabytes
import java.util.Locale
import kotlinx.coroutines.launch
import uniffi.allreads_ffi.AvailableDictionary
import uniffi.allreads_ffi.InstalledDictionary
import uniffi.allreads_ffi.VocabStats

/** Step 23c: download and remove offline dictionaries. */
@Composable
fun DictionarySection(dictionaries: Dictionaries) {
    val list by dictionaries.list.collectAsStateWithLifecycle()
    val downloads by dictionaries.downloads.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { dictionaries.refresh() }

    // Published dictionaries, plus installed ones no longer published.
    val pairs = (list.available.orEmpty().map { it.pair } + list.installed.map { it.pair }).distinct().sorted()
    if (list.loading && (pairs.isEmpty() || list.fromCache)) {
        // A slow first answer is common on phones: show that something is happening, and each new try.
        Row(verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            CircularProgressIndicator(Modifier.size(18.dp), strokeWidth = 2.dp)
            Column {
                Text(
                    if (pairs.isEmpty()) AppText.get(R.string.looking_for_dictionaries) else AppText.get(R.string.checking_for_new_dictionaries),
                    style = MaterialTheme.typography.bodyMedium,
                )
                Text(
                    if (list.attempt > 1) AppText.get(R.string.still_trying, list.attempt) else AppText.get(R.string.this_can_take_a_while),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
    } else if (pairs.isEmpty() && list.listError == null) {
        Text(
            if (list.available == null) AppText.get(R.string.looking_for_dictionaries) else AppText.get(R.string.no_dictionaries_yet),
            style = MaterialTheme.typography.bodyMedium,
        )
    }
    pairs.forEach { pair ->
        DictionaryRow(
            dictionaries = dictionaries,
            pair = pair,
            available = list.available?.firstOrNull { it.pair == pair },
            installed = list.installed.firstOrNull { it.pair == pair },
            download = downloads[pair],
        )
    }
    list.listError?.takeIf { !list.loading }?.let {
        val scope = rememberCoroutineScope()
        Row(verticalAlignment = Alignment.CenterVertically) {
            Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error, modifier = Modifier.weight(1f))
            TextButton(onClick = { scope.launch { dictionaries.refresh() } }) { Text(stringResource(R.string.try_again)) }
        }
    }
    Text(
        stringResource(R.string.definitions_from_wiktionary_via_kaikki),
        style = MaterialTheme.typography.bodySmall,
    )
}

@Composable
private fun DictionaryRow(
    dictionaries: Dictionaries,
    pair: String,
    available: AvailableDictionary?,
    installed: InstalledDictionary?,
    download: DownloadState?,
) {
    val scope = rememberCoroutineScope()
    DownloadRow(
        name = pairName(pair),
        download = download,
        installedText = installed?.let {
            stringResource(R.string.installed_built, megabytes(it.bytes.toLong()), it.built)
        },
        availableText = available?.let {
            AppText.get(R.string.download_on_the_phone, megabytes(it.downloadBytes.toLong()), megabytes(it.installedBytes.toLong()))
        },
        downloadBytes = available?.downloadBytes?.toLong() ?: 0,
        onDownload = { available?.let(dictionaries::download) },
        onCancel = { dictionaries.cancel(pair) },
        onRemove = { scope.launch { runCatching { dictionaries.remove(pair) } } },
    )
}

/**
 * The book lists for "Find a book": one per language, downloaded like a
 * dictionary. Shown only when one is published or installed.
 */
@Composable
fun CatalogSection(catalogs: Catalogs) {
    val list by catalogs.list.collectAsStateWithLifecycle()
    val downloads by catalogs.downloads.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    LaunchedEffect(Unit) { catalogs.refresh() }
    val languages = (list.available.orEmpty().map { it.languageCode } + list.installed.map { it.languageCode })
        .distinct()
        .sorted()
    if (languages.isEmpty()) return
    Text(stringResource(R.string.book_lists), style = MaterialTheme.typography.titleMedium)
    languages.forEach { language ->
        val available = list.available?.firstOrNull { it.languageCode == language }
        val installed = list.installed.firstOrNull { it.languageCode == language }
        DownloadRow(
            name = Locale.forLanguageTag(language).getDisplayLanguage(Locale.ENGLISH),
            download = downloads[language],
            installedText = installed?.let {
                pluralStringResource(R.plurals.book_list_installed, it.books.toInt(), it.books.toInt(), megabytes(it.bytes.toLong()), it.built)
            },
            availableText = available?.let {
                pluralStringResource(R.plurals.book_list_available, it.books.toInt(), it.books.toInt(), megabytes(it.downloadBytes.toLong()))
            },
            downloadBytes = available?.downloadBytes?.toLong() ?: 0,
            onDownload = { available?.let(catalogs::download) },
            onCancel = { catalogs.cancel(language) },
            onRemove = { scope.launch { runCatching { catalogs.remove(language) } } },
        )
    }
    Text(stringResource(R.string.book_lists_explained), style = MaterialTheme.typography.bodySmall)
}

/**
 * One downloadable file in Settings: its progress while downloading, its size
 * and a Remove button once installed, else a Download button (asking first on
 * mobile data). `installedText` and `availableText` are null when it is not
 * installed or not published.
 */
@Composable
private fun DownloadRow(
    name: String,
    download: DownloadState?,
    installedText: String?,
    availableText: String?,
    downloadBytes: Long,
    onDownload: () -> Unit,
    onCancel: () -> Unit,
    onRemove: () -> Unit,
) {
    val context = LocalContext.current
    var confirmMobileData by remember { mutableStateOf(false) }

    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(name, style = MaterialTheme.typography.bodyLarge)
        when {
            download is DownloadState.Running -> {
                val total = download.total.coerceAtLeast(1)
                LinearProgressIndicator(
                    progress = { (download.bytes.toFloat() / total).coerceIn(0f, 1f) },
                    modifier = Modifier.fillMaxWidth(),
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        stringResource(R.string.downloading_of, megabytes(download.bytes), megabytes(download.total)),
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.weight(1f),
                    )
                    TextButton(onClick = onCancel) { Text(stringResource(R.string.cancel)) }
                }
            }
            installedText != null -> {
                Text(installedText, style = MaterialTheme.typography.bodySmall)
                OutlinedButton(onClick = onRemove) { Text(stringResource(R.string.remove)) }
            }
            availableText != null -> {
                Text(availableText, style = MaterialTheme.typography.bodySmall)
                if (download is DownloadState.Failed) {
                    Text(download.message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
                }
                Button(onClick = { if (onMobileData(context)) confirmMobileData = true else onDownload() }) {
                    Text(stringResource(if (download is DownloadState.Failed) R.string.try_again else R.string.download))
                }
            }
        }
    }

    if (confirmMobileData) {
        AlertDialog(
            onDismissRequest = { confirmMobileData = false },
            title = { Text(stringResource(R.string.download_on_mobile_data)) },
            text = { Text(stringResource(R.string.is_a_download, name, megabytes(downloadBytes))) },
            confirmButton = {
                TextButton(onClick = {
                    confirmMobileData = false
                    onDownload()
                }) { Text(stringResource(R.string.download)) }
            },
            dismissButton = { TextButton(onClick = { confirmMobileData = false }) { Text(stringResource(R.string.not_now)) } },
        )
    }
}

/** "French → English" for `fr-en`. */
fun pairName(pair: String): String =
    pair.split('-').joinToString(" → ") { Locale.forLanguageTag(it).getDisplayLanguage(Locale.ENGLISH) }


/** True on a metered connection (usually mobile data). */
fun onMobileData(context: Context): Boolean =
    context.getSystemService(ConnectivityManager::class.java)?.isActiveNetworkMetered ?: false

/**
 * The vocabulary check per installed dictionary with word frequencies: start
 * it, and once done, how many words it marked and Undo. `refresh` changes
 * when a check closes, so the counts are read again.
 */
@Composable
fun VocabCheckSection(services: Services, refresh: Int, onStart: (String) -> Unit) {
    val list by services.dictionaries.list.collectAsStateWithLifecycle()
    val scope = rememberCoroutineScope()
    // Language → words the check marked (still untouched), for dictionaries with frequencies.
    var checks by remember { mutableStateOf<Map<String, Int>>(emptyMap()) }
    // Language → how its checks held up while reading.
    var stats by remember { mutableStateOf<Map<String, VocabStats>>(emptyMap()) }
    var askUndo by remember { mutableStateOf<String?>(null) }
    var undoError by remember { mutableStateOf<String?>(null) }
    suspend fun reload() {
        checks = list.installed.map { it.pair.substringBefore('-') }.distinct()
            .filter { runCatching { services.repository.vocabularyCheckAvailable(it) }.getOrDefault(false) }
            .associateWith { runCatching { services.repository.vocabularyCheckCount(it) }.getOrDefault(0) }
        stats = checks.keys.associateWith { runCatching { services.repository.vocabularyCheckStats(it) }.getOrNull() }
            .filterValues { it != null }
            .mapValues { it.value!! }
    }
    LaunchedEffect(list.installed, refresh) { reload() }
    if (checks.isEmpty()) return
    Text(stringResource(R.string.vocabulary_check), style = MaterialTheme.typography.titleMedium)
    checks.forEach { (language, marked) ->
        val name = Locale.forLanguageTag(language).getDisplayLanguage(Locale.ENGLISH)
        Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Text(stringResource(R.string.vocab_check_settings_explained, name), style = MaterialTheme.typography.bodySmall)
            if (marked > 0) {
                Text(
                    pluralStringResource(R.plurals.marked_by_the_check, marked, "%,d".format(marked)),
                    style = MaterialTheme.typography.bodySmall,
                )
            }
            // How it held up: marks the reader set back while reading.
            stats[language]?.takeIf { it.marked > 0u }?.let { held ->
                Text(
                    pluralStringResource(
                        R.plurals.check_corrections,
                        held.marked.toInt(),
                        "%,d".format(held.corrected.toInt()),
                        "%,d".format(held.marked.toInt()),
                    ),
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedButton(onClick = { onStart(language) }) { Text(stringResource(R.string.check_your_vocabulary)) }
                if (marked > 0) {
                    TextButton(onClick = { askUndo = language }) { Text(stringResource(R.string.undo)) }
                }
            }
        }
    }
    undoError?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error) }
    askUndo?.let { language ->
        AlertDialog(
            onDismissRequest = { askUndo = null },
            title = { Text(stringResource(R.string.undo_vocabulary_check_question)) },
            text = { Text(stringResource(R.string.undo_vocabulary_check_text)) },
            confirmButton = {
                TextButton(onClick = {
                    askUndo = null
                    scope.launch {
                        undoError = runCatching { services.repository.undoVocabularyCheck(language) }
                            .exceptionOrNull()
                            ?.let { AppText.get(R.string.undo_failed, describe(it)) }
                        reload()
                    }
                }) { Text(stringResource(R.string.undo)) }
            },
            dismissButton = { TextButton(onClick = { askUndo = null }) { Text(stringResource(R.string.cancel)) } },
        )
    }
}
