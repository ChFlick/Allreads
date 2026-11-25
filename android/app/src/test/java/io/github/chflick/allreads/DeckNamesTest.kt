package io.github.chflick.allreads

import io.github.chflick.allreads.anki.newDeckName
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class DeckNamesTest {
    @Test
    fun aTopLevelDeckIsJustItsName() = assertEquals("Allreads", newDeckName(null, " Allreads "))

    @Test
    fun aSubdeckIsJoinedToItsParent() =
        assertEquals("Languages::French::Allreads", newDeckName("Languages::French", "Allreads"))

    @Test
    fun strayPartsAreDropped() = assertEquals("Languages::Books", newDeckName("Languages", ":: Books ::"))

    @Test
    fun aBlankNameIsRefused() {
        assertNull(newDeckName("Languages", "  "))
        assertNull(newDeckName(null, "::"))
    }
}
