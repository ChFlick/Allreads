package io.github.chflick.allreads.data

import java.io.File
import kotlinx.coroutines.CancellationException
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
import uniffi.allreads_ffi.AvailableCatalog
import uniffi.allreads_ffi.BookSummary
import uniffi.allreads_ffi.CatalogBook
import uniffi.allreads_ffi.CatalogWord
import uniffi.allreads_ffi.InstalledCatalog

/** The Rust side of book catalogs: installing, listing and removing. */
interface CatalogFiles {
    suspend fun installedCatalogs(): List<InstalledCatalog>
    suspend fun installCatalog(download: File, sha256: String, languageCode: String): InstalledCatalog
    suspend fun removeCatalog(languageCode: String)
    fun parseCatalogManifest(json: String): List<AvailableCatalog>
}

/** What "Find a book" reads from an installed catalog, and how it imports a book. */
interface BookCatalog {
    /** Every book with its fit to the saved words, most popular first. */
    suspend fun catalogBooks(languageCode: String): List<CatalogBook>
    suspend fun searchCatalog(languageCode: String, query: String): List<Long>
    suspend fun catalogNewWords(languageCode: String, bookId: Long, limit: Int): List<CatalogWord>
    suspend fun newBookPath(name: String?, extension: String): String
    suspend fun importBook(path: String, languageCode: String): BookSummary
}

/** What Settings lists: catalogs that can be downloaded, and those installed. */
data class CatalogList(
    /** `null` while loading or when the list could not be fetched. */
    val available: List<AvailableCatalog>? = null,
    val installed: List<InstalledCatalog> = emptyList(),
)

/**
 * Downloads book catalogs for "Find a book", one per language, from the same
 * published list as the dictionaries. Like a dictionary download, it runs in
 * the app's scope and an interrupted download is deleted.
 */
class Catalogs(
    private val files: CatalogFiles,
    private val fetcher: Fetcher,
    private val cacheDir: File,
    private val scope: CoroutineScope,
    private val describe: (Throwable) -> String,
) {
    private val _list = MutableStateFlow(CatalogList())
    val list: StateFlow<CatalogList> = _list.asStateFlow()

    private val _downloads = MutableStateFlow<Map<String, DownloadState>>(emptyMap())

    /** Running or failed downloads by language; a finished one is removed. */
    val downloads: StateFlow<Map<String, DownloadState>> = _downloads.asStateFlow()

    private val jobs = mutableMapOf<String, Job>()

    /** Re-reads the installed catalogs and the published list. */
    suspend fun refresh() {
        val installed = runCatching { files.installedCatalogs() }.getOrDefault(emptyList())
        _list.update { it.copy(installed = installed) }
        val available = runCatching {
            withContext(Dispatchers.IO) { files.parseCatalogManifest(fetcher.text(DICTIONARY_MANIFEST_URL)) }
        }
        _list.update { it.copy(available = available.getOrNull() ?: it.available) }
    }

    /** Starts downloading; does nothing if this language's catalog is already downloading. */
    fun download(catalog: AvailableCatalog) {
        val language = catalog.languageCode
        synchronized(jobs) {
            val previous = jobs[language]
            if (previous != null && previous.isActive && !previous.isCancelled) return
            jobs[language] = scope.launch {
                previous?.join()
                run(catalog)
            }
        }
    }

    /** Waits until the current download for `languageCode` (if any) has finished. */
    suspend fun awaitDownload(languageCode: String) {
        synchronized(jobs) { jobs[languageCode] }?.join()
    }

    private suspend fun run(catalog: AvailableCatalog) {
        val language = catalog.languageCode
        val total = catalog.downloadBytes.toLong()
        setState(language, DownloadState.Running(0, total))
        val target = File(cacheDir, "catalog-$language.sqlite.gz.download")
        try {
            withContext(Dispatchers.IO) {
                fetcher.download(catalog.url, target) { bytes ->
                    ensureActive()
                    setState(language, DownloadState.Running(bytes, total))
                }
            }
            files.installCatalog(target, catalog.sha256, language)
            target.delete()
            setState(language, null)
            refresh()
        } catch (e: Exception) {
            target.delete()
            if (e is CancellationException) {
                setState(language, null)
                throw e
            }
            setState(language, DownloadState.Failed(describe(e)))
        }
    }

    fun cancel(languageCode: String) {
        synchronized(jobs) { jobs[languageCode]?.cancel() }
        setState(languageCode, null)
    }

    suspend fun remove(languageCode: String) {
        files.removeCatalog(languageCode)
        refresh()
    }

    private fun setState(language: String, state: DownloadState?) =
        _downloads.update { if (state == null) it - language else it + (language to state) }
}
