package io.github.chflick.allreads

import io.github.chflick.allreads.anki.AnkiNoteData
import io.github.chflick.allreads.anki.DeckImport
import org.junit.Assert.assertEquals
import org.junit.Test
import uniffi.allreads_ffi.CardProgress

class DeckImportTest {
    @Test
    fun notesAreGroupedByNoteTypeWithTheirProgress() {
        val anki = FakeAnki()
        anki.typeNames[10] = "Basic"
        anki.fieldLists[10] = listOf("Front", "Back")
        anki.deckNotes += listOf(
            AnkiNoteData(1, 10, listOf("le chat", "cat")),
            AnkiNoteData(2, 10, listOf("chien", "dog")),
            AnkiNoteData(3, 10, listOf("oiseau", "bird")),
            AnkiNoteData(4, 20, listOf("maison", "house", "f.")),
        )
        val deck = "deck:\"Français\""
        anki.searches["$deck -is:suspended"] = setOf(1, 2, 4)
        anki.searches["$deck prop:ivl>=21"] = setOf(1)
        anki.searches["$deck prop:ivl>=7"] = setOf(1, 2)
        anki.searches["$deck is:review"] = setOf(1, 2)
        val groups = DeckImport(anki).read("Français")
        assertEquals(listOf("Basic", "Note type 20"), groups.map { it.name })
        assertEquals(listOf("Front", "Back"), groups[0].fields)
        assertEquals(listOf("Field 1", "Field 2", "Field 3"), groups[1].fields)
        assertEquals(
            listOf(CardProgress.Interval(21u), CardProgress.Interval(7u), CardProgress.Suspended),
            groups[0].notes.map { it.progress },
        )
        assertEquals(CardProgress.New, groups[1].notes[0].progress)
    }
}
