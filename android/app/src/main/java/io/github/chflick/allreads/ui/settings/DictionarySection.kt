package io.github.chflick.allreads.ui.settings

import io.github.chflick.allreads.ui.AppText
import androidx.compose.ui.res.pluralStringResource
import androidx.compose.ui.res.stringResource
import io.github.chflick.allreads.R
import android.content.Context
import android.net.ConnectivityManager
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.LinearProgressIndicator
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.chflick.allreads.data.Catalogs
import io.github.chflick.allreads.data.Dictionaries
import io.github.chflick.allreads.data.DownloadState
import io.github.chflick.allreads.ui.megabytes
import java.util.Locale
import kotlinx.coroutines.launch
import uniffi.allreads_ffi.AvailableDictionary
import uniffi.allreads_ffi.InstalledDictionary

/** Step 23c: download and remove offline dictionaries. */
@Composable
fun DictionarySection(dictionaries: Dictionaries) {
    val list by dictionaries.list.collectAsStateWithLifecycle()
    val downloads by dictionaries.downloads.collectAsStateWithLifecycle()
    LaunchedEffect(Unit) { dictionaries.refresh() }

    // Published dictionaries, plus installed ones no longer published.
    val pairs = (list.available.orEmpty().map { it.pair } + list.installed.map { it.pair }).distinct().sorted()
    if (pairs.isEmpty()) {
        Text(
            if (list.available == null && list.listError == null) AppText.get(R.string.looking_for_dictionaries) else AppText.get(R.string.no_dictionaries_yet),
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
    list.listError?.let {
        Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
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
