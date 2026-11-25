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

    override fun noteTypeIds(): Set<Long> = api.modelList.orEmpty().keys

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

    companion object {
        const val PERMISSION: String = AddContentApi.READ_WRITE_PERMISSION
        const val PACKAGE = "com.ichi2.anki"
    }
}
