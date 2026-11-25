package io.github.chflick.allreads.anki

import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import uniffi.allreads_ffi.AnkiNoteType

/** Where the note type ids live, and where their definitions come from (Rust). */
interface NoteTypeStore {
    suspend fun modelId(languageCode: String): Long?
    suspend fun setModelId(languageCode: String, modelId: Long)
    suspend fun noteType(languageCode: String, includeWordCard: Boolean): AnkiNoteType
}

/**
 * Step 17: makes sure AnkiDroid has the "Allreads (Language)" note type. It is
 * created on first use and again if it was deleted in Anki; an existing one is
 * never changed, so edits made in Anki survive. If the stored id is lost (the
 * app was reinstalled), a note type with the same name is reused rather than
 * duplicated.
 */
class AnkiNoteTypes(private val bridge: AnkiBridge, private val store: NoteTypeStore) {
    // Settings and sync may ask at the same time; only one may create.
    private val lock = Mutex()

    /** The note type id for a language, creating the note type if needed; `null` if AnkiDroid refused. */
    suspend fun ensure(languageCode: String, deckId: Long, includeWordCard: Boolean): Long? = lock.withLock {
        val existing = bridge.noteTypes()
        val known = store.modelId(languageCode)
        if (known != null && known in existing) return@withLock known
        val noteType = store.noteType(languageCode, includeWordCard)
        val id = existing.entries.firstOrNull { it.value == noteType.name }?.key
            ?: bridge.addNoteType(noteType, deckId)
            ?: return@withLock null
        store.setModelId(languageCode, id)
        id
    }
}
