package io.github.chflick.allreads.data

import io.github.chflick.allreads.anki.AnkiBridge
import io.github.chflick.allreads.anki.AnkiNoteTypes

/** Everything the screens need, created once by the application. */
class Services(
    val repository: Repository,
    val settings: SettingsStore,
    val apiKeys: ApiKeyStore,
    val anki: AnkiBridge,
) {
    val ankiNoteTypes = AnkiNoteTypes(anki, repository)
}
