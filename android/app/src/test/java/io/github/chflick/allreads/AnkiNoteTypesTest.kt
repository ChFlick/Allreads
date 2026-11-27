package io.github.chflick.allreads

import io.github.chflick.allreads.anki.AnkiAccess
import io.github.chflick.allreads.anki.AnkiBridge
import io.github.chflick.allreads.anki.AnkiNoteData
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

    /** Notes by id: note type, deck, fields, tags. */
    data class Note(val noteTypeId: Long, val deckId: Long, val fields: List<String>, val tags: Set<String>)
    val notes: MutableMap<Long, Note> = java.util.concurrent.ConcurrentHashMap()
    var access = AnkiAccess.Connected
    var failAdds = false

    override fun access() = access
    override fun addNote(noteTypeId: Long, deckId: Long, fields: List<String>, tags: Set<String>): Long? {
        if (failAdds) return null
        return nextId.getAndIncrement().also { notes[it] = Note(noteTypeId, deckId, fields, tags) }
    }
    override fun updateNote(noteId: Long, fields: List<String>, tags: Set<String>): Boolean {
        val note = notes[noteId] ?: return false
        notes[noteId] = note.copy(fields = fields, tags = tags)
        return true
    }
    override fun findNote(noteTypeId: Long, word: String): Long? =
        notes.entries.firstOrNull { it.value.noteTypeId == noteTypeId && it.value.fields.first() == word }?.key
    override fun decks() = listOf(AnkiDeck(1, "Default"))
    override fun addDeck(name: String): Long? = if (refuse) null else nextId.getAndIncrement()
    override fun noteTypes(): Map<Long, String> = noteTypes.mapValues { it.value.name } + typeNames
    override fun addNoteType(noteType: AnkiNoteType, deckId: Long): Long? {
        if (refuse) return null
        return nextId.getAndIncrement().also { noteTypes[it] = noteType }
    }

    /** Notes for imports, and the ids each Anki search returns. */
    val deckNotes = mutableListOf<AnkiNoteData>()
    val searches = mutableMapOf<String, Set<Long>>()
    val fieldLists = mutableMapOf<Long, List<String>>()
    val typeNames = mutableMapOf<Long, String>()
    override fun fieldNames(noteTypeId: Long) = fieldLists[noteTypeId].orEmpty()
    override fun searchNotes(query: String) = deckNotes.toList()
    override fun searchNoteIds(query: String) = searches[query].orEmpty()

    /** Notes whose cards were suspended. */
    val suspended = mutableSetOf<Long>()
    override fun suspendNote(noteId: Long, tag: String): Boolean {
        if (noteId !in notes) return true
        suspended += noteId
        notes[noteId] = notes.getValue(noteId).let { it.copy(tags = it.tags + tag) }
        return true
    }
    override fun noteTags(noteId: Long) = notes[noteId]?.tags
    override fun deleteNotes(noteIds: Collection<Long>) = noteIds.count { notes.remove(it) != null }
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
