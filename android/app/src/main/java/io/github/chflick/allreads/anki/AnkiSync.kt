package io.github.chflick.allreads.anki

import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import uniffi.allreads_ffi.AnkiNote

/** The Rust side of syncing: the queue and where pushes are recorded. */
interface SyncStore {
    suspend fun deckId(): Long?
    suspend fun pendingNotes(): List<AnkiNote>
    suspend fun recordPush(note: AnkiNote, noteId: Long, deckId: Long)
    suspend fun recordFailure(note: AnkiNote)
}

/** What a drain did. */
sealed interface SyncResult {
    /** AnkiDroid is missing, not allowed, or no deck is chosen; nothing was tried. */
    data class NotReady(val reason: AnkiAccess?, val waiting: Int) : SyncResult
    data class Done(val pushed: Int, val failed: Int) : SyncResult
}

/**
 * Step 18: pushes every queued word to AnkiDroid. A word with a card updates
 * that card; a word without one is matched to an existing note with the same
 * Word first (e.g. after forgetting and re-saving it), and only added if there
 * is none, so cards are never duplicated. A card deleted in Anki is added
 * again. Failures stay queued for the next drain.
 */
class AnkiSync(
    private val bridge: AnkiBridge,
    private val noteTypes: AnkiNoteTypes,
    private val store: SyncStore,
    private val includeWordCard: () -> Boolean,
) {
    // Two drains at once could add the same word twice.
    private val lock = Mutex()

    suspend fun drain(): SyncResult = lock.withLock {
        val access = bridge.access()
        val deckId = store.deckId()
        if (access != AnkiAccess.Connected || deckId == null) {
            return@withLock SyncResult.NotReady(access.takeIf { it != AnkiAccess.Connected }, store.pendingNotes().size)
        }
        var pushed = 0
        var failed = 0
        for (note in store.pendingNotes()) {
            val noteId = runCatching { push(note, deckId) }.getOrNull()
            if (noteId != null) {
                store.recordPush(note, noteId, deckId)
                pushed++
            } else {
                store.recordFailure(note)
                failed++
            }
        }
        SyncResult.Done(pushed, failed)
    }

    /** The note id the word now has in AnkiDroid, or `null` on failure. */
    private suspend fun push(note: AnkiNote, deckId: Long): Long? {
        val tags = note.tags.toSet()
        note.noteId?.let { linked ->
            if (bridge.updateNote(linked, note.fields, tags)) return linked
        }
        val noteTypeId = noteTypes.ensure(note.languageCode, deckId, includeWordCard()) ?: return null
        val word = note.fields.firstOrNull() ?: return null
        bridge.findNote(noteTypeId, word)?.let { existing ->
            if (bridge.updateNote(existing, note.fields, tags)) return existing
        }
        return bridge.addNote(noteTypeId, deckId, note.fields, tags)
    }
}
