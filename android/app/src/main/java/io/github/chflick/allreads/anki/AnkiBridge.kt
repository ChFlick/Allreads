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
}
