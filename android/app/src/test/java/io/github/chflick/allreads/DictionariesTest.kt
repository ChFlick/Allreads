package io.github.chflick.allreads

import io.github.chflick.allreads.data.DictionaryFiles
import io.github.chflick.allreads.data.Dictionaries
import io.github.chflick.allreads.data.DownloadState
import io.github.chflick.allreads.data.Fetcher
import java.io.File
import java.io.IOException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import uniffi.allreads_ffi.AvailableDictionary
import uniffi.allreads_ffi.InstalledDictionary

class DictionariesTest {
    @get:Rule
    val temp = TemporaryFolder()

    private val french = AvailableDictionary(
        pair = "fr-en", languageCode = "fr", url = "https://example.org/fr-en.sqlite.gz",
        built = "2026-11-25", downloadBytes = 10u, installedBytes = 40u, sha256 = "abc",
    )

    /** Stands in for Rust: remembers what was installed and what the download held. */
    private inner class FakeFiles : DictionaryFiles {
        val installed = mutableListOf<InstalledDictionary>()
        var received: String? = null
        var refuse: Exception? = null
        override suspend fun installed() = installed.toList()
        override suspend fun install(download: File, sha256: String, pair: String): InstalledDictionary {
            refuse?.let { throw it }
            received = download.readText()
            return InstalledDictionary(pair, "2026-11-25", 40u).also { installed += it }
        }
        override suspend fun remove(pair: String) {
            installed.removeAll { it.pair == pair }
        }
        override fun parseManifest(json: String) = if (json == "manifest") listOf(french) else error("bad")
    }

    private class FakeFetcher(var fail: Boolean = false) : Fetcher {
        override fun text(url: String) = if (fail) throw IOException("offline") else "manifest"
        override fun download(url: String, target: File, progress: (Long) -> Unit) {
            target.writeText("half")
            progress(4)
            if (fail) throw IOException("connection lost")
            target.appendText(" and the rest")
            progress(17)
        }
    }

    private val files = FakeFiles()
    private val fetcher = FakeFetcher()
    private val dictionaries by lazy {
        Dictionaries(files, fetcher, temp.root, CoroutineScope(SupervisorJob() + Dispatchers.Default)) {
            it.message ?: "error"
        }
    }

    private fun awaitInstalled(pair: String) = runBlocking {
        withTimeout(5_000) { dictionaries.list.first { list -> list.installed.any { it.pair == pair } } }
    }

    private fun awaitFailure(pair: String) = runBlocking {
        withTimeout(5_000) { dictionaries.downloads.first { it[pair] is DownloadState.Failed } }
    }

    @Test
    fun theListComesFromTheManifest() = runBlocking {
        dictionaries.refresh()
        assertEquals(listOf(french), dictionaries.list.value.available)
        assertEquals(french, dictionaries.availableFor("fr"))
        assertEquals(null, dictionaries.availableFor("de"))
    }

    @Test
    fun offlineTheListSaysSoAndKeepsWhatIsInstalled() = runBlocking {
        files.installed += InstalledDictionary("fr-en", "2026-11-25", 40u)
        fetcher.fail = true
        dictionaries.refresh()
        assertEquals(1, dictionaries.list.value.installed.size)
        assertEquals(null, dictionaries.list.value.available)
        assertTrue(dictionaries.list.value.listError != null)
    }

    @Test
    fun aDownloadIsInstalledAndItsTemporaryFileDeleted() = runBlocking {
        dictionaries.download(french)
        awaitInstalled("fr-en")
        assertEquals("half and the rest", files.received)
        assertEquals(listOf("fr-en"), dictionaries.list.value.installed.map { it.pair })
        assertTrue(temp.root.listFiles().orEmpty().isEmpty())
    }

    @Test
    fun anInterruptedDownloadFailsAndLeavesNothing() = runBlocking {
        fetcher.fail = true
        dictionaries.download(french)
        awaitFailure("fr-en")
        assertEquals(DownloadState.Failed("connection lost"), dictionaries.downloads.value["fr-en"])
        assertEquals(null, files.received)
        assertTrue(temp.root.listFiles().orEmpty().isEmpty())
    }

    @Test
    fun aRefusedFileIsReportedAndDeleted() = runBlocking {
        files.refuse = IllegalStateException("damaged")
        dictionaries.download(french)
        awaitFailure("fr-en")
        assertEquals(DownloadState.Failed("damaged"), dictionaries.downloads.value["fr-en"])
        assertTrue(temp.root.listFiles().orEmpty().isEmpty())
    }

    @Test
    fun removingUpdatesTheList() = runBlocking {
        dictionaries.download(french)
        awaitInstalled("fr-en")
        dictionaries.remove("fr-en")
        assertTrue(dictionaries.list.value.installed.isEmpty())
    }
}
