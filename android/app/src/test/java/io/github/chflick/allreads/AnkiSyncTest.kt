package io.github.chflick.allreads

import io.github.chflick.allreads.anki.AnkiAccess
import io.github.chflick.allreads.anki.FORGOTTEN_TAG
import io.github.chflick.allreads.anki.AnkiNoteTypes
import io.github.chflick.allreads.anki.AnkiSync
import io.github.chflick.allreads.anki.SyncResult
import io.github.chflick.allreads.anki.SyncStore
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.allreads_ffi.AnkiNote

/** The Rust queue, in memory: a word stays queued until a push is recorded. */
class FakeQueue : SyncStore {
    var deck: Long? = 1
    val queue = linkedMapOf<Long, AnkiNote>()
    val links = mutableMapOf<Long, Long>()
    val failures = mutableMapOf<Long, Int>()

    fun save(termId: Long, word: String, translation: String) {
        queue[termId] = AnkiNote(termId, queueId = termId, languageCode = "fr", noteId = links[termId],
            fields = listOf(word, "", translation, "<b>$word</b>", "", ""), tags = listOf("allreads", "fr"))
    }

    override suspend fun deckId() = deck
    override suspend fun pendingNotes() = queue.values.toList()
    override suspend fun waitingCount() = queue.size
    override suspend fun recordPush(note: AnkiNote, noteId: Long, deckId: Long) {
        links[note.termId] = noteId
        queue.remove(note.termId)
    }
    override suspend fun recordFailure(note: AnkiNote) {
        failures.merge(note.termId, 1, Int::plus)
    }

    /** Note ids of forgotten words waiting to be suspended. */
    val suspends = mutableListOf<Long>()
    override suspend fun pendingSuspends(minAgeSeconds: Int) = suspends.toList()
    override suspend fun recordSuspended(noteId: Long) {
        suspends.remove(noteId)
    }
}

class AnkiSyncTest {
    private val anki = FakeAnki()
    private val queue = FakeQueue()
    private val sync = AnkiSync(anki, AnkiNoteTypes(anki, FakeStore()), queue) { false }

    @Test
    fun aSavedWordBecomesOneCardInTheChosenDeck() = runBlocking {
        queue.save(1, "homme", "man")
        assertEquals(SyncResult.Done(pushed = 1, failed = 0), sync.drain())
        val note = anki.notes.values.single()
        assertEquals(1L, note.deckId)
        assertEquals("man", note.fields[2])
        assertEquals(setOf("allreads", "fr"), note.tags)
        assert(queue.queue.isEmpty())
    }

    @Test
    fun editingAWordUpdatesTheSameCard() = runBlocking {
        queue.save(1, "homme", "man")
        sync.drain()
        val noteId = queue.links.getValue(1)
        queue.save(1, "homme", "human")
        sync.drain()
        assertEquals(1, anki.notes.size)
        assertEquals("human", anki.notes.getValue(noteId).fields[2])
    }

    @Test
    fun aCardDeletedInAnkiIsAddedAgain() = runBlocking {
        queue.save(1, "homme", "man")
        sync.drain()
        anki.notes.clear()
        queue.save(1, "homme", "man")
        sync.drain()
        assertEquals(1, anki.notes.size)
        assertEquals(anki.notes.keys.single(), queue.links[1])
    }

    @Test
    fun aWordForgottenAndSavedAgainReusesItsCard() = runBlocking {
        queue.save(1, "homme", "man")
        sync.drain()
        // Forgetting deletes the term and its link; saving it again makes a new term.
        queue.save(2, "homme", "human")
        sync.drain()
        assertEquals(1, anki.notes.size)
        assertEquals(queue.links[1], queue.links[2])
        assertEquals("human", anki.notes.values.single().fields[2])
    }

    @Test
    fun nothingIsTriedWithoutAccessOrADeck() = runBlocking {
        queue.save(1, "homme", "man")
        anki.access = AnkiAccess.NeedsPermission
        assertEquals(SyncResult.NotReady(AnkiAccess.NeedsPermission, waiting = 1), sync.drain())
        anki.access = AnkiAccess.Connected
        queue.deck = null
        assertEquals(SyncResult.NotReady(null, waiting = 1), sync.drain())
        assert(anki.notes.isEmpty())
        assert(queue.failures.isEmpty())
    }

    @Test
    fun aFailedPushStaysQueuedAndGoesThroughLater() = runBlocking {
        queue.save(1, "homme", "man")
        anki.failAdds = true
        assertEquals(SyncResult.Done(pushed = 0, failed = 1), sync.drain())
        assertEquals(1, queue.failures[1])
        assertEquals(1, sync.waiting.value)
        anki.failAdds = false
        assertEquals(SyncResult.Done(pushed = 1, failed = 0), sync.drain())
        assertEquals(1, anki.notes.size)
        assertEquals(0, sync.waiting.value)
    }

    @Test
    fun aForgottenWordsCardIsSuspendedAndTagged() = runBlocking {
        queue.save(1, "chat", "cat")
        sync.drain()
        val noteId = queue.links.getValue(1)
        // The word is forgotten in Allreads: the core queues its card.
        queue.suspends += noteId
        sync.drain()
        assertEquals(setOf(noteId), anki.suspended)
        assertEquals(true, anki.noteTags(noteId)?.contains(FORGOTTEN_TAG))
        assertEquals(emptyList<Long>(), queue.suspends)
    }

    @Test
    fun aWordSavedAgainAfterItsCardWasSuspendedGetsAFreshCard() = runBlocking {
        queue.save(1, "chat", "cat")
        sync.drain()
        val old = queue.links.getValue(1)
        queue.suspends += old
        sync.drain()
        // Saved again later as a new word (forgetting removed its link).
        queue.links.remove(2)
        queue.save(2, "chat", "cat")
        sync.drain()
        val fresh = queue.links.getValue(2)
        assertEquals(false, fresh == old)
        assertEquals(null, anki.noteTags(old))
        assertEquals(false, anki.noteTags(fresh)?.contains(FORGOTTEN_TAG))
    }
}
