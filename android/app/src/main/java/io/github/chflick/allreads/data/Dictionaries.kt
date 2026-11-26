package io.github.chflick.allreads.data

import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import uniffi.allreads_ffi.AvailableDictionary
import uniffi.allreads_ffi.InstalledDictionary

/** Where the app finds the list of dictionaries. */
const val DICTIONARY_MANIFEST_URL =
    "https://github.com/ChFlick/Allreads/releases/download/dictionaries/dictionaries.json"

/** Fetching over HTTPS; an interface so the JVM tests can use files instead. */
/** A fetched web page as it arrived: its bytes and the `Content-Type` header, for the core to decode. */
class FetchedPage(val bytes: ByteArray, val contentType: String?)

interface Fetcher {
    /** A UTF-8 text file, such as the dictionary list. */
    fun text(url: String): String

    /** A web page, undecoded: the core works out its character set. */
    fun page(url: String): FetchedPage = FetchedPage(text(url).toByteArray(), "text/html; charset=utf-8")

    /** Writes `url` to `target`, calling `progress` with the bytes written so far. */
    fun download(url: String, target: File, progress: (Long) -> Unit)
}

/** The Rust side: installing, listing and removing. */
interface DictionaryFiles {
    suspend fun installed(): List<InstalledDictionary>
    suspend fun install(download: File, sha256: String, pair: String): InstalledDictionary
    suspend fun remove(pair: String)
    fun parseManifest(json: String): List<AvailableDictionary>
}

/** A dictionary's download, as Settings and the word drawer show it. */
sealed interface DownloadState {
    data class Running(val bytes: Long, val total: Long) : DownloadState
    data class Failed(val message: String) : DownloadState
}

/** What Settings lists: what can be downloaded, and what is installed. */
data class DictionaryList(
    /** `null` while loading or when the list could not be fetched. */
    val available: List<AvailableDictionary>? = null,
    val installed: List<InstalledDictionary> = emptyList(),
    val listError: String? = null,
)

/**
 * Step 23c: downloads dictionaries on demand. A download runs in the app's
 * scope, so it continues if the screen changes; if it is interrupted, the
 * partial file is deleted and the next attempt starts over.
 */
class Dictionaries(
    private val files: DictionaryFiles,
    private val fetcher: Fetcher,
    private val cacheDir: File,
    private val scope: CoroutineScope,
    private val describe: (Throwable) -> String,
) {
    private val _list = MutableStateFlow(DictionaryList())
    val list: StateFlow<DictionaryList> = _list.asStateFlow()

    private val _downloads = MutableStateFlow<Map<String, DownloadState>>(emptyMap())

    /** Running or failed downloads by pair; a finished one is removed. */
    val downloads: StateFlow<Map<String, DownloadState>> = _downloads.asStateFlow()

    private val jobs = mutableMapOf<String, Job>()

    /** Re-reads the installed files and the published list. */
    suspend fun refresh() {
        val installed = runCatching { files.installed() }.getOrDefault(emptyList())
        _list.update { it.copy(installed = installed) }
        val available = runCatching {
            withContext(Dispatchers.IO) { files.parseManifest(fetcher.text(DICTIONARY_MANIFEST_URL)) }
        }
        _list.update {
            it.copy(
                available = available.getOrNull() ?: it.available,
                listError = available.exceptionOrNull()?.let { "Could not load the list of dictionaries." },
            )
        }
    }

    /** The published dictionary for a book language, if there is one. */
    fun availableFor(languageCode: String): AvailableDictionary? =
        _list.value.available?.firstOrNull { it.languageCode == languageCode }

    /** Starts downloading; does nothing if this dictionary is already downloading. */
    fun download(dictionary: AvailableDictionary) {
        val pair = dictionary.pair
        synchronized(jobs) {
            val previous = jobs[pair]
            if (previous != null && previous.isActive && !previous.isCancelled) return
            jobs[pair] = scope.launch {
                // A cancelled download may still be blocked in a read; let it end
                // first, so two downloads never write the same file.
                previous?.join()
                run(dictionary)
            }
        }
    }

    /** Waits until the current download of `pair` (if any) has finished, either way. */
    suspend fun awaitDownload(pair: String) {
        synchronized(jobs) { jobs[pair] }?.join()
    }

    private suspend fun run(dictionary: AvailableDictionary) {
        val pair = dictionary.pair
        val total = dictionary.downloadBytes.toLong()
        setState(pair, DownloadState.Running(0, total))
        val target = File(cacheDir, "$pair.sqlite.gz.download")
        try {
            withContext(Dispatchers.IO) {
                fetcher.download(dictionary.url, target) { bytes ->
                    ensureActive()
                    setState(pair, DownloadState.Running(bytes, total))
                }
            }
            files.install(target, dictionary.sha256, pair)
            target.delete()
            setState(pair, null)
            refresh()
        } catch (e: Exception) {
            // Delete before reporting, so nothing half-downloaded is left once the state says so.
            target.delete()
            if (e is kotlinx.coroutines.CancellationException) {
                setState(pair, null)
                throw e
            }
            setState(pair, DownloadState.Failed(describe(e)))
        }
    }

    fun cancel(pair: String) {
        // The job stays in the map until it ends, so a new download waits for it.
        synchronized(jobs) { jobs[pair]?.cancel() }
        setState(pair, null)
    }

    suspend fun remove(pair: String) {
        files.remove(pair)
        refresh()
    }

    private fun setState(pair: String, state: DownloadState?) =
        _downloads.update { if (state == null) it - pair else it + (pair to state) }
}

/** [Fetcher] with `HttpURLConnection`. */
class HttpFetcher : Fetcher {
    override fun text(url: String): String = open(url).inputStream.use { it.readBytes().decodeToString() }

    override fun page(url: String): FetchedPage {
        val connection = open(url)
        return FetchedPage(connection.inputStream.use { it.readBytes() }, connection.contentType)
    }

    override fun download(url: String, target: File, progress: (Long) -> Unit) {
        val connection = open(url)
        connection.inputStream.use { input ->
            target.outputStream().use { output ->
                val buffer = ByteArray(64 * 1024)
                var written = 0L
                var lastReport = 0L
                while (true) {
                    val read = input.read(buffer)
                    if (read < 0) break
                    output.write(buffer, 0, read)
                    written += read
                    // Report every 256 KB, not every buffer, to keep the UI calm.
                    if (written - lastReport >= 256 * 1024) {
                        progress(written)
                        lastReport = written
                    }
                }
                progress(written)
            }
        }
    }

    private fun open(url: String): HttpURLConnection {
        var address = URL(url)
        // HttpURLConnection follows redirects only within one protocol, so an
        // "http://" link that moved to "https://" is followed here.
        repeat(MAX_REDIRECTS) {
            val connection = address.openConnection() as HttpURLConnection
            connection.connectTimeout = 15_000
            connection.readTimeout = 30_000
            // GitHub answers release downloads with a redirect to its file host.
            connection.instanceFollowRedirects = true
            // Some sites refuse Java's default agent. A generic one, nothing about the user.
            connection.setRequestProperty("User-Agent", "Mozilla/5.0 (Linux; Android) Allreads")
            val code = connection.responseCode
            val location = connection.getHeaderField("Location")
            if (code in 300..399 && location != null) {
                connection.disconnect()
                address = URL(address, location)
                return@repeat
            }
            if (code !in 200..299) {
                connection.disconnect()
                throw IOException("The server answered $code")
            }
            return connection
        }
        throw IOException("Too many redirects")
    }

    private companion object {
        const val MAX_REDIRECTS = 5
    }
}
