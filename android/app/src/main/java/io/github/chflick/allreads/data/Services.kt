package io.github.chflick.allreads.data

import io.github.chflick.allreads.anki.AnkiBridge
import io.github.chflick.allreads.anki.AnkiNoteTypes
import io.github.chflick.allreads.anki.AnkiSync
import io.github.chflick.allreads.speech.Speech
import io.github.chflick.allreads.ui.describe
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.launch

/** Something shared to the app: a web page's address, or text with an optional title. */
sealed interface SharedImport {
    data class Link(val url: String) : SharedImport
    data class Text(val title: String?, val text: String) : SharedImport

    companion object {
        private val url = Regex("""https?://\S+""")

        /** A shared text that holds a link becomes a [Link] (browsers often add the page title); else [Text]. */
        fun from(text: String?, subject: String?): SharedImport? {
            val shared = text?.trim().orEmpty()
            if (shared.isEmpty()) return null
            url.find(shared)?.let { match ->
                // Only the link and maybe a title: import the page, not the words around the link.
                if (shared.length - match.value.length < 200) return Link(trimLink(match.value))
            }
            return Text(subject?.takeIf { it.isNotBlank() }, shared)
        }

        /**
         * Drops punctuation that ends the sentence around a link, not the link:
         * "(see https://x.org/a)." loses ")." but ".../Paris_(mythology)" keeps its ")".
         */
        private fun trimLink(link: String): String {
            var end = link.length
            while (end > 0) {
                val last = link[end - 1]
                val unbalanced = last == ')' && link.take(end).count { it == ')' } > link.take(end).count { it == '(' }
                if (last == '.' || last == ',' || unbalanced) end-- else break
            }
            return link.take(end)
        }
    }
}

/** Everything the screens need, created once by the application. */
class Services(
    val repository: Repository,
    /** For the dictionary downloads' temporary files. */
    cacheDir: java.io.File,
    val settings: SettingsStore,
    val apiKeys: ApiKeyStore,
    val anki: AnkiBridge,
    val speech: Speech,
) {
    val ankiNoteTypes = AnkiNoteTypes(anki, repository)
    val ankiSync = AnkiSync(anki, ankiNoteTypes, repository) { settings.settings.value.ankiWordCard }

    // Lives as long as the process, so a sync started by a save finishes even if the screen closes.
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    val dictionaries = Dictionaries(repository, HttpFetcher(), cacheDir, scope, ::describe)

    /** A link or text shared to the app ("Share → Allreads"), waiting for the library to import it. */
    val sharedImport = MutableStateFlow<SharedImport?>(null)

    /** Pushes queued words to AnkiDroid in the background; words that fail stay queued. */
    fun syncAnki() {
        scope.launch { runCatching { ankiSync.drain() } }
    }
}
