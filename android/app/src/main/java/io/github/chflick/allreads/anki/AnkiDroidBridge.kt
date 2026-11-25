package io.github.chflick.allreads.anki

import android.content.Context
import android.content.pm.PackageManager
import androidx.core.content.ContextCompat
import com.ichi2.anki.api.AddContentApi
import uniffi.allreads_ffi.AnkiNoteType

/** [AnkiBridge] through AnkiDroid's API library (`AddContentApi`). */
class AnkiDroidBridge(context: Context) : AnkiBridge {
    private val context = context.applicationContext
    private val api by lazy { AddContentApi(this.context) }

    override fun access(): AnkiAccess = when {
        AddContentApi.getAnkiDroidPackageName(context) == null -> AnkiAccess.NotInstalled
        !hasPermission() -> AnkiAccess.NeedsPermission
        else -> AnkiAccess.Connected
    }

    private fun hasPermission(): Boolean {
        // Very old AnkiDroid versions do not define the permission and need none.
        val defined = runCatching { context.packageManager.getPermissionInfo(PERMISSION, 0) }.isSuccess
        return !defined ||
            ContextCompat.checkSelfPermission(context, PERMISSION) == PackageManager.PERMISSION_GRANTED
    }

    override fun decks(): List<AnkiDeck> =
        api.deckList.orEmpty().map { (id, name) -> AnkiDeck(id, name) }.sortedBy { it.name.lowercase() }

    override fun addDeck(name: String): Long? = api.addNewDeck(name)

    override fun noteTypes(): Map<Long, String> = api.modelList.orEmpty()

    override fun addNoteType(noteType: AnkiNoteType, deckId: Long): Long? = api.addNewCustomModel(
        noteType.name,
        noteType.fields.toTypedArray(),
        noteType.cards.map { it.name }.toTypedArray(),
        noteType.cards.map { it.front }.toTypedArray(),
        noteType.cards.map { it.back }.toTypedArray(),
        noteType.css,
        deckId,
        // Sort the browser by the Word field.
        0,
    )

    override fun addNote(noteTypeId: Long, deckId: Long, fields: List<String>, tags: Set<String>): Long? =
        api.addNote(noteTypeId, deckId, fields.toTypedArray(), tags)

    override fun updateNote(noteId: Long, fields: List<String>, tags: Set<String>): Boolean {
        // AnkiDroid answers a note deleted in Anki with false, or on some versions an exception.
        if (api.getNote(noteId) == null) return false
        return runCatching { api.updateNoteFields(noteId, fields.toTypedArray()) }.getOrDefault(false) &&
            runCatching { api.updateNoteTags(noteId, tags) }.getOrDefault(false)
    }

    override fun findNote(noteTypeId: Long, word: String): Long? =
        api.findDuplicateNotes(noteTypeId, word)?.firstOrNull()?.id

    companion object {
        const val PERMISSION: String = AddContentApi.READ_WRITE_PERMISSION
        const val PACKAGE = "com.ichi2.anki"
    }
}
