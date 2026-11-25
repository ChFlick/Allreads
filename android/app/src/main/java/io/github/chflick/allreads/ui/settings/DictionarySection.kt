package io.github.chflick.allreads.ui.settings

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
import io.github.chflick.allreads.data.Dictionaries
import io.github.chflick.allreads.data.DownloadState
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
            if (list.available == null && list.listError == null) "Looking for dictionaries…" else "No dictionaries yet.",
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
        "Definitions from Wiktionary, via kaikki.org, under CC BY-SA 4.0. Looked up on the phone; nothing is sent anywhere.",
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
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var confirmMobileData by remember { mutableStateOf(false) }

    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Text(pairName(pair), style = MaterialTheme.typography.bodyLarge)
        when {
            download is DownloadState.Running -> {
                val total = download.total.coerceAtLeast(1)
                LinearProgressIndicator(
                    progress = { (download.bytes.toFloat() / total).coerceIn(0f, 1f) },
                    modifier = Modifier.fillMaxWidth(),
                )
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text(
                        "Downloading ${megabytes(download.bytes)} of ${megabytes(download.total)}",
                        style = MaterialTheme.typography.bodySmall,
                        modifier = Modifier.weight(1f),
                    )
                    TextButton(onClick = { dictionaries.cancel(pair) }) { Text("Cancel") }
                }
            }
            installed != null -> {
                Text(
                    "Installed · ${megabytes(installed.bytes.toLong())} · built ${installed.built}",
                    style = MaterialTheme.typography.bodySmall,
                )
                OutlinedButton(onClick = { scope.launch { runCatching { dictionaries.remove(pair) } } }) {
                    Text("Remove")
                }
            }
            available != null -> {
                Text(
                    "${megabytes(available.downloadBytes.toLong())} download, " +
                        "${megabytes(available.installedBytes.toLong())} on the phone",
                    style = MaterialTheme.typography.bodySmall,
                )
                if (download is DownloadState.Failed) {
                    Text(download.message, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
                }
                Button(onClick = {
                    if (onMobileData(context)) confirmMobileData = true else dictionaries.download(available)
                }) { Text(if (download is DownloadState.Failed) "Try again" else "Download") }
            }
        }
    }

    if (confirmMobileData && available != null) {
        AlertDialog(
            onDismissRequest = { confirmMobileData = false },
            title = { Text("Download on mobile data?") },
            text = { Text("${pairName(pair)} is a ${megabytes(available.downloadBytes.toLong())} download.") },
            confirmButton = {
                TextButton(onClick = {
                    confirmMobileData = false
                    dictionaries.download(available)
                }) { Text("Download") }
            },
            dismissButton = { TextButton(onClick = { confirmMobileData = false }) { Text("Not now") } },
        )
    }
}

/** "French → English" for `fr-en`. */
fun pairName(pair: String): String =
    pair.split('-').joinToString(" → ") { Locale.forLanguageTag(it).getDisplayLanguage(Locale.ENGLISH) }

private fun megabytes(bytes: Long): String = "%.1f MB".format(Locale.ENGLISH, bytes / 1_000_000.0)

/** True on a metered connection (usually mobile data). */
fun onMobileData(context: Context): Boolean =
    context.getSystemService(ConnectivityManager::class.java)?.isActiveNetworkMetered ?: false
