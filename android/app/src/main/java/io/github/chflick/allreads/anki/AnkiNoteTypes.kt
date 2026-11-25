package io.github.chflick.allreads.anki

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
 * never changed, so edits made in Anki survive.
 */
class AnkiNoteTypes(private val bridge: AnkiBridge, private val store: NoteTypeStore) {
    /** The note type id for a language, creating the note type if needed; `null` if AnkiDroid refused. */
    suspend fun ensure(languageCode: String, deckId: Long, includeWordCard: Boolean): Long? {
        val known = store.modelId(languageCode)
        if (known != null && known in bridge.noteTypeIds()) return known
        val created = bridge.addNoteType(store.noteType(languageCode, includeWordCard), deckId) ?: return null
        store.setModelId(languageCode, created)
        return created
    }
}
