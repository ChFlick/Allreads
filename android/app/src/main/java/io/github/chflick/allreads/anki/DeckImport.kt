package io.github.chflick.allreads.anki

import io.github.chflick.allreads.ui.AppText
import io.github.chflick.allreads.R
import uniffi.allreads_ffi.CardProgress
import uniffi.allreads_ffi.ImportGroup
import uniffi.allreads_ffi.ImportNote

/**
 * Reads an AnkiDroid deck for importing words: its notes grouped by note type,
 * each with its review progress.
 *
 * AnkiDroid's API gives no intervals, so progress comes from Anki searches,
 * one per step of the core's rule (`prop:ivl>=21`, `>=14`, `>=7`, `>=3`,
 * `is:review`), and `-is:suspended` for the notes with a card still active.
 * A note counts with its furthest card, as for `.apkg` files. If the searches
 * fail, progress is left out and the import offers one level for all.
 *
 * [typeLabel] and [fieldLabel] name note types and fields AnkiDroid gives no
 * name for; tests pass plain ones, since they run without the app's resources.
 */
class DeckImport(
    private val bridge: AnkiBridge,
    private val typeLabel: (Long) -> String = { AppText.get(R.string.note_type, it) },
    private val fieldLabel: (Int) -> String = { AppText.get(R.string.field_n, it) },
) {
    fun read(deckName: String): List<ImportGroup> {
        val deck = "deck:\"${deckName.replace("\"", "\\\"")}\""
        val notes = bridge.searchNotes(deck)
        val progress = runCatching { progressOf(deck) }.getOrNull()
        val typeNames = bridge.noteTypes()
        return notes.groupBy { it.noteTypeId }
            .map { (typeId, typeNotes) ->
                val width = typeNotes.maxOf { it.fields.size }
                ImportGroup(
                    name = typeNames[typeId] ?: typeLabel(typeId),
                    fields = bridge.fieldNames(typeId).ifEmpty { (1..width).map(fieldLabel) },
                    notes = typeNotes.map { note -> ImportNote(note.fields, progress?.invoke(note.id)) },
                )
            }
            // The note type with the most notes first: usually the one to import.
            .sortedByDescending { it.notes.size }
    }

    /** Each note's progress from the bucket searches. */
    private fun progressOf(deck: String): (Long) -> CardProgress {
        val active = bridge.searchNoteIds("$deck -is:suspended")
        val buckets = listOf(21, 14, 7, 3).map { days -> days.toUInt() to bridge.searchNoteIds("$deck prop:ivl>=$days") }
        val reviewed = bridge.searchNoteIds("$deck is:review")
        return { id -> progressFor(id, active, buckets, reviewed) }
    }

    companion object {
        /** A note's progress: suspended if no card is active, else its furthest card's interval bucket, else new. */
        fun progressFor(
            id: Long,
            active: Set<Long>,
            buckets: List<Pair<UInt, Set<Long>>>,
            reviewed: Set<Long>,
        ): CardProgress = when {
            id !in active -> CardProgress.Suspended
            else -> buckets.firstOrNull { (_, ids) -> id in ids }?.let { CardProgress.Interval(it.first) }
                ?: if (id in reviewed) CardProgress.Interval(1u) else CardProgress.New
        }
    }
}
