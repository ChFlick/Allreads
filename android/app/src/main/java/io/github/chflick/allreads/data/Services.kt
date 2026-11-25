package io.github.chflick.allreads.data

import io.github.chflick.allreads.anki.AnkiBridge
import io.github.chflick.allreads.anki.AnkiNoteTypes
import io.github.chflick.allreads.anki.AnkiSync
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch

/** Everything the screens need, created once by the application. */
class Services(
    val repository: Repository,
    val settings: SettingsStore,
    val apiKeys: ApiKeyStore,
    val anki: AnkiBridge,
) {
    val ankiNoteTypes = AnkiNoteTypes(anki, repository)
    val ankiSync = AnkiSync(anki, ankiNoteTypes, repository) { settings.settings.value.ankiWordCard }

    // Lives as long as the process, so a sync started by a save finishes even if the screen closes.
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    /** Pushes queued words to AnkiDroid in the background; words that fail stay queued. */
    fun syncAnki() {
        scope.launch { runCatching { ankiSync.drain() } }
    }
}
