package io.github.chflick.allreads.anki

import android.content.ContentValues
import android.content.Context
import android.net.Uri
import android.content.pm.PackageManager
import androidx.core.content.ContextCompat
import com.ichi2.anki.FlashCardsContract
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

    override fun fieldNames(noteTypeId: Long): List<String> =
        runCatching { api.getFieldList(noteTypeId)?.toList() }.getOrNull().orEmpty()

    override fun searchNotes(query: String): List<AnkiNoteData> = query(query, withFields = true)

    override fun searchNoteIds(query: String): Set<Long> = query(query, withFields = false).map { it.id }.toSet()

    override fun suspendNote(noteId: Long, tag: String): Boolean {
        // getNote gives null both for a deleted note and when AnkiDroid fails to
        // answer, so ask first whether the note is there: only a note that is
        // really gone may drop the queued suspension.
        when (noteExists(noteId)) {
            null -> return false
            false -> return true
            true -> Unit
        }
        val note = api.getNote(noteId) ?: return false
        val resolver = context.contentResolver
        val cards = Uri.withAppendedPath(Uri.withAppendedPath(FlashCardsContract.Note.CONTENT_URI, noteId.toString()), "cards")
        val ords = resolver.query(cards, arrayOf(FlashCardsContract.Card.CARD_ORD), null, null, null)?.use { cursor ->
            buildList<Int> { while (cursor.moveToNext()) add(cursor.getInt(0)) }
        } ?: return false
        for (ord in ords) {
            val values = ContentValues().apply {
                put(FlashCardsContract.ReviewInfo.NOTE_ID, noteId)
                put(FlashCardsContract.ReviewInfo.CARD_ORD, ord)
                put(FlashCardsContract.ReviewInfo.SUSPEND, 1)
            }
            // A card AnkiDroid did not suspend: try again on the next drain, and don't tag the note yet.
            if (resolver.update(FlashCardsContract.ReviewInfo.CONTENT_URI, values, null, null) < 1) return false
        }
        return runCatching { api.updateNoteTags(noteId, note.tags + tag) }.getOrDefault(false)
    }

    override fun noteTags(noteId: Long): Set<String>? = api.getNote(noteId)?.tags

    /** Whether AnkiDroid has the note, or `null` when it did not answer. */
    private fun noteExists(noteId: Long): Boolean? = runCatching {
        // A search, because a search for a missing note answers with no rows,
        // while the note's own URI may throw for it.
        context.contentResolver.query(FlashCardsContract.Note.CONTENT_URI, arrayOf(FlashCardsContract.Note._ID), "nid:$noteId", null, null)
            ?.use { it.count > 0 }
    }.getOrNull()

    override fun deleteNotes(noteIds: Collection<Long>): Int = noteIds.sumOf { id ->
        runCatching {
            context.contentResolver.delete(Uri.withAppendedPath(FlashCardsContract.Note.CONTENT_URI, id.toString()), null, null)
        }.getOrDefault(0)
    }

    /** Notes through AnkiDroid's content provider, which takes an Anki search as its selection. */
    private fun query(search: String, withFields: Boolean): List<AnkiNoteData> {
        val projection = if (withFields) {
            arrayOf(FlashCardsContract.Note._ID, FlashCardsContract.Note.MID, FlashCardsContract.Note.FLDS)
        } else {
            arrayOf(FlashCardsContract.Note._ID)
        }
        val cursor = context.contentResolver.query(FlashCardsContract.Note.CONTENT_URI, projection, search, null, null)
            ?: return emptyList()
        return cursor.use {
            buildList {
                while (it.moveToNext()) {
                    add(
                        AnkiNoteData(
                            id = it.getLong(0),
                            noteTypeId = if (withFields) it.getLong(1) else 0,
                            // Fields are separated by the "unit separator" character.
                            fields = if (withFields) it.getString(2).orEmpty().split('\u001f') else emptyList(),
                        ),
                    )
                }
            }
        }
    }

    companion object {
        const val PERMISSION: String = AddContentApi.READ_WRITE_PERMISSION
        const val PACKAGE = "com.ichi2.anki"
    }
}
