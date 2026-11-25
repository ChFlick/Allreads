package io.github.chflick.allreads.anki

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
 * tested on the JVM with a fake; [AnkiDroidBridge] is the real one.
 */
interface AnkiBridge {
    fun access(): AnkiAccess

    /** Every deck, sorted by name. */
    fun decks(): List<AnkiDeck>

    /** Creates a deck and returns its id, or `null` if AnkiDroid refused. */
    fun addDeck(name: String): Long?
}
