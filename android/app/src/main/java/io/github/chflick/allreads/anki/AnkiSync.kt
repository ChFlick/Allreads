package io.github.chflick.allreads.anki

import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import uniffi.allreads_ffi.AnkiNote

/** The Rust side of syncing: the queue and where pushes are recorded. */
interface SyncStore {
    suspend fun deckId(): Long?
    suspend fun pendingNotes(): List<AnkiNote>
    suspend fun waitingCount(): Int
    suspend fun recordPush(note: AnkiNote, noteId: Long, deckId: Long)
    suspend fun recordFailure(note: AnkiNote)

    /** Cards of words forgotten at least `minAgeSeconds` ago, to suspend now. */
    suspend fun pendingSuspends(minAgeSeconds: Int): List<Long>
    suspend fun recordSuspended(noteId: Long)
}

/** How long a forgotten word's card waits before it is suspended: the Undo snackbar's time, and some. */
const val SUSPEND_AFTER_SECONDS = 60

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

    private val _waiting = MutableStateFlow<Int?>(null)

    /** Words still queued after the last drain or count; `null` before the first. */
    val waiting: StateFlow<Int?> = _waiting.asStateFlow()

    /** Re-reads the queue size, e.g. when Settings opens. */
    suspend fun countWaiting() {
        _waiting.value = store.waitingCount()
    }

    suspend fun drain(): SyncResult = lock.withLock {
        try {
            drainLocked()
        } finally {
            _waiting.value = runCatching { store.waitingCount() }.getOrNull()
        }
    }

    private suspend fun drainLocked(): SyncResult {
        val access = bridge.access()
        if (access == AnkiAccess.Connected) suspendForgotten()
        val deckId = store.deckId()
        if (access != AnkiAccess.Connected || deckId == null) {
            return SyncResult.NotReady(access.takeIf { it != AnkiAccess.Connected }, store.waitingCount())
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
        return SyncResult.Done(pushed, failed)
    }

    /**
     * Suspends the cards of words forgotten in Allreads (before pushes, so a
     * word saved again afterwards meets an already suspended card, see [push]).
     */
    private suspend fun suspendForgotten() {
        for (noteId in store.pendingSuspends(SUSPEND_AFTER_SECONDS)) {
            if (runCatching { bridge.suspendNote(noteId, FORGOTTEN_TAG) }.getOrDefault(false)) store.recordSuspended(noteId)
        }
    }

    /**
     * A note whose word was forgotten and whose cards were suspended. AnkiDroid
     * cannot un-suspend through its API, so a word saved again gets a fresh
     * card instead, and the suspended one is deleted.
     */
    private fun isForgotten(noteId: Long): Boolean = bridge.noteTags(noteId)?.contains(FORGOTTEN_TAG) == true

    /** The note id the word now has in AnkiDroid, or `null` on failure. */
    private suspend fun push(note: AnkiNote, deckId: Long): Long? {
        val tags = note.tags.toSet()
        note.noteId?.let { linked ->
            if (isForgotten(linked)) bridge.deleteNotes(listOf(linked))
            else if (bridge.updateNote(linked, note.fields, tags)) return linked
        }
        val noteTypeId = noteTypes.ensure(note.languageCode, deckId, includeWordCard()) ?: return null
        val word = note.fields.firstOrNull() ?: return null
        bridge.findNote(noteTypeId, word)?.let { existing ->
            if (isForgotten(existing)) bridge.deleteNotes(listOf(existing))
            else if (bridge.updateNote(existing, note.fields, tags)) return existing
        }
        return bridge.addNote(noteTypeId, deckId, note.fields, tags)
    }
}
