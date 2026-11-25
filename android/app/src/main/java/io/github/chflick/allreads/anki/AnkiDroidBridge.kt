package io.github.chflick.allreads.anki

import android.content.ContentValues
import android.content.Context
import android.content.pm.PackageManager
import android.net.Uri
import androidx.core.content.ContextCompat

/**
 * Talks to AnkiDroid through its ContentProvider, the API AnkiDroid documents
 * for other apps (`FlashCardsContract`). AnkiDroid's own `api` library is a
 * thin wrapper over the same calls; it is not on Maven Central, so we call the
 * provider directly instead (see docs/DECISIONS.md).
 */
class AnkiDroidBridge(context: Context) : AnkiBridge {
    private val context = context.applicationContext
    private val resolver = context.contentResolver

    override fun access(): AnkiAccess = when {
        context.packageManager.resolveContentProvider(AUTHORITY, 0) == null -> AnkiAccess.NotInstalled
        !hasPermission() -> AnkiAccess.NeedsPermission
        else -> AnkiAccess.Connected
    }

    private fun hasPermission(): Boolean {
        // Very old AnkiDroid versions do not define the permission and need none.
        val defined = runCatching { context.packageManager.getPermissionInfo(PERMISSION, 0) }.isSuccess
        return !defined ||
            ContextCompat.checkSelfPermission(context, PERMISSION) == PackageManager.PERMISSION_GRANTED
    }

    override fun decks(): List<AnkiDeck> {
        val decks = mutableListOf<AnkiDeck>()
        resolver.query(DECKS_URI, arrayOf(DECK_ID, DECK_NAME), null, null, null)?.use { cursor ->
            val id = cursor.getColumnIndexOrThrow(DECK_ID)
            val name = cursor.getColumnIndexOrThrow(DECK_NAME)
            while (cursor.moveToNext()) decks += AnkiDeck(cursor.getLong(id), cursor.getString(name))
        }
        return decks.sortedBy { it.name.lowercase() }
    }

    override fun addDeck(name: String): Long? {
        val values = ContentValues().apply { put(DECK_NAME, name) }
        return resolver.insert(DECKS_URI, values)?.lastPathSegment?.toLongOrNull()
    }

    companion object {
        const val PERMISSION = "com.ichi2.anki.permission.READ_WRITE_DATABASE"
        const val PACKAGE = "com.ichi2.anki"
        private const val AUTHORITY = "com.ichi2.anki.flashcards"
        private val DECKS_URI: Uri = Uri.parse("content://$AUTHORITY/decks")
        private const val DECK_ID = "deck_id"
        private const val DECK_NAME = "deck_name"
    }
}
