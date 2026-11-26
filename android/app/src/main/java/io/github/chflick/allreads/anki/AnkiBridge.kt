package io.github.chflick.allreads.anki

import uniffi.allreads_ffi.AnkiNoteType

/** Whether the app can talk to AnkiDroid right now. */
enum class AnkiAccess {
    /** AnkiDroid is not installed (or too old to share its cards). */
    NotInstalled,

    /** AnkiDroid is installed, but the user has not allowed access yet. */
    NeedsPermission,
    Connected,
}

data class AnkiDeck(val id: Long, val name: String)

/** A note as read for importing words: its id, note type and field values. */
data class AnkiNoteData(val id: Long, val noteTypeId: Long, val fields: List<String>)

/**
 * Everything the app asks of AnkiDroid. An interface, so the sync logic can be
 * tested on the JVM with a fake; [AnkiDroidBridge] is the real one. Calls
 * other than [access] need [AnkiAccess.Connected] and run off the main thread.
 */
interface AnkiBridge {
    fun access(): AnkiAccess

    /** Every deck, sorted by name. */
    fun decks(): List<AnkiDeck>

    /** Creates a deck and returns its id, or `null` if AnkiDroid refused. */
    fun addDeck(name: String): Long?

    /** Every note type in AnkiDroid: id to name. */
    fun noteTypes(): Map<Long, String>

    /** Creates a note type, with `deckId` as its default deck; `null` if AnkiDroid refused. */
    fun addNoteType(noteType: AnkiNoteType, deckId: Long): Long?

    /** Adds a note and returns its id, or `null` if AnkiDroid refused. */
    fun addNote(noteTypeId: Long, deckId: Long, fields: List<String>, tags: Set<String>): Long?

    /** Replaces a note's fields and tags; `false` if the note no longer exists. */
    fun updateNote(noteId: Long, fields: List<String>, tags: Set<String>): Boolean

    /** A note of this type whose first field (Word) is `word`, if one exists. */
    fun findNote(noteTypeId: Long, word: String): Long?

    /** A note type's field names, in order; empty if it does not exist. */
    fun fieldNames(noteTypeId: Long): List<String>

    /** The notes matching an Anki search, e.g. `deck:"French"` (subdecks included). */
    fun searchNotes(query: String): List<AnkiNoteData>

    /** The ids of the notes matching an Anki search, e.g. `deck:"French" prop:ivl>=21`. */
    fun searchNoteIds(query: String): Set<Long>
}
