package io.github.chflick.allreads.data

import io.github.chflick.allreads.ui.AppText
import io.github.chflick.allreads.R
import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
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
    /** True while the published list is being fetched. */
    val loading: Boolean = false,
    /** Which try of the fetch is running (1, 2, 3), for "Still trying" on a slow connection. */
    val attempt: Int = 0,
    /** True when [available] is the copy saved last time, not yet confirmed by this fetch. */
    val fromCache: Boolean = false,
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
    /** Pauses before each retry of a failed list fetch. */
    private val retryPauses: List<Long> = listOf(1_000, 3_000),
) {
    private val _list = MutableStateFlow(DictionaryList())
    val list: StateFlow<DictionaryList> = _list.asStateFlow()

    private val _downloads = MutableStateFlow<Map<String, DownloadState>>(emptyMap())

    /** Running or failed downloads by pair; a finished one is removed. */
    val downloads: StateFlow<Map<String, DownloadState>> = _downloads.asStateFlow()

    private val jobs = mutableMapOf<String, Job>()

    /** One fetch of the list at a time: callers that arrive during one wait for it and share its result. */
    private val listing = Mutex()

    /** The list as last fetched, so it shows at once next time, even before the network answers. */
    private val savedList = File(cacheDir, "dictionaries.json")

    /** How many fetches of the list have finished. */
    @Volatile private var fetches = 0

    /**
     * Re-reads the installed files and the published list. The list is tried
     * a few times, as the first request after the app starts often fails or
     * hangs on a phone; an error shows only once every try has failed.
     */
    suspend fun refresh() {
        // Read before anything waits, so a fetch that finishes meanwhile counts as shared.
        val started = fetches
        val installed = runCatching { files.installed() }.getOrDefault(emptyList())
        _list.update { it.copy(installed = installed) }
        loadSaved()
        listing.withLock {
            // Another caller fetched the list while this one waited: use theirs.
            if (fetches != started && _list.value.available != null && !_list.value.fromCache) return
            _list.update { it.copy(loading = true, attempt = 1, listError = null) }
            var available = fetchList()
            for ((index, pause) in retryPauses.withIndex()) {
                if (available.isSuccess) break
                delay(pause)
                _list.update { it.copy(attempt = index + 2) }
                available = fetchList()
            }
            fetches += 1
            val fetched = available.getOrNull()
            _list.update {
                it.copy(
                    available = fetched ?: it.available,
                    fromCache = fetched == null && it.fromCache,
                    // With a saved list on screen, a failed fetch is not worth an error.
                    listError = available.exceptionOrNull()
                        ?.takeIf { _ -> it.available == null }
                        ?.let { AppText.get(R.string.could_not_load_dictionaries) },
                    loading = false,
                    attempt = 0,
                )
            }
        }
    }

    /** Shows the list saved last time while nothing newer is known. */
    private suspend fun loadSaved() {
        if (_list.value.available != null) return
        val saved = runCatching {
            withContext(Dispatchers.IO) { if (savedList.exists()) files.parseManifest(savedList.readText()) else null }
        }.getOrNull() ?: return
        _list.update { if (it.available == null) it.copy(available = saved, fromCache = true) else it }
    }

    private suspend fun fetchList(): Result<List<AvailableDictionary>> = runCatching {
        withContext(Dispatchers.IO) {
            val json = fetcher.text(DICTIONARY_MANIFEST_URL)
            val parsed = files.parseManifest(json)
            runCatching { savedList.writeText(json) }
            parsed
        }
    }.onFailure { if (it is kotlinx.coroutines.CancellationException) throw it }

    /**
     * Makes sure a list is known (e.g. Settings was never opened): the saved
     * one at once, refreshed in the background, or else waits for the fetch.
     */
    suspend fun ensureListed() {
        if (_list.value.available != null) return
        loadSaved()
        if (_list.value.available == null) refresh() else scope.launch { refresh() }
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
