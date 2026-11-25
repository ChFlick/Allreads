package io.github.chflick.allreads

import io.github.chflick.allreads.anki.AnkiAccess
import io.github.chflick.allreads.anki.AnkiBridge
import io.github.chflick.allreads.anki.AnkiDeck
import io.github.chflick.allreads.anki.AnkiNoteTypes
import io.github.chflick.allreads.anki.NoteTypeStore
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import uniffi.allreads_ffi.AnkiNoteType

/** A stand-in AnkiDroid that keeps its note types in memory. */
class FakeAnki : AnkiBridge {
    val noteTypes: MutableMap<Long, AnkiNoteType> = java.util.concurrent.ConcurrentHashMap()
    var refuse = false
    private val nextId = java.util.concurrent.atomic.AtomicLong(100)

    override fun access() = AnkiAccess.Connected
    override fun decks() = listOf(AnkiDeck(1, "Default"))
    override fun addDeck(name: String): Long? = if (refuse) null else nextId.getAndIncrement()
    override fun noteTypes(): Map<Long, String> = noteTypes.mapValues { it.value.name }
    override fun addNoteType(noteType: AnkiNoteType, deckId: Long): Long? {
        if (refuse) return null
        return nextId.getAndIncrement().also { noteTypes[it] = noteType }
    }
}

class FakeStore : NoteTypeStore {
    val ids: MutableMap<String, Long> = java.util.concurrent.ConcurrentHashMap()
    override suspend fun modelId(languageCode: String) = ids[languageCode]
    override suspend fun setModelId(languageCode: String, modelId: Long) {
        ids[languageCode] = modelId
    }
    override suspend fun noteType(languageCode: String, includeWordCard: Boolean) = AnkiNoteType(
        name = "Allreads ($languageCode)",
        fields = listOf("Word"),
        cards = emptyList(),
        css = "",
    )
}

class AnkiNoteTypesTest {
    private val anki = FakeAnki()
    private val store = FakeStore()
    private val noteTypes = AnkiNoteTypes(anki, store)

    @Test
    fun createsTheNoteTypeOnceAndRemembersIt() = runBlocking {
        val id = noteTypes.ensure("fr", deckId = 1, includeWordCard = false)
        assertEquals(id, noteTypes.ensure("fr", deckId = 1, includeWordCard = false))
        assertEquals(1, anki.noteTypes.size)
        assertEquals(id, store.ids["fr"])
        assertEquals("Allreads (fr)", anki.noteTypes[id]?.name)
    }

    @Test
    fun recreatesANoteTypeDeletedInAnki() = runBlocking {
        val first = noteTypes.ensure("fr", 1, false)
        anki.noteTypes.clear()
        val second = noteTypes.ensure("fr", 1, false)
        assertEquals(1, anki.noteTypes.size)
        assertEquals(second, store.ids["fr"])
        assert(first != second)
    }

    @Test
    fun languagesGetTheirOwnNoteType() = runBlocking {
        noteTypes.ensure("fr", 1, false)
        noteTypes.ensure("de", 1, false)
        assertEquals(2, anki.noteTypes.size)
    }

    @Test
    fun aNoteTypeLeftFromAnEarlierInstallIsReused() = runBlocking {
        val old = noteTypes.ensure("fr", 1, false)
        store.ids.clear()
        assertEquals(old, noteTypes.ensure("fr", 1, false))
        assertEquals(1, anki.noteTypes.size)
    }

    @Test
    fun concurrentCallsCreateOneNoteType() = runBlocking {
        val ids = (1..5).map { async(Dispatchers.Default) { noteTypes.ensure("fr", 1, false) } }.awaitAll()
        assertEquals(1, ids.toSet().size)
        assertEquals(1, anki.noteTypes.size)
    }

    @Test
    fun aRefusalStoresNothing() = runBlocking {
        anki.refuse = true
        assertNull(noteTypes.ensure("fr", 1, false))
        assertNull(store.ids["fr"])
    }
}
