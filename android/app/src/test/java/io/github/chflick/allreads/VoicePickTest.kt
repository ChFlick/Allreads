package io.github.chflick.allreads

import io.github.chflick.allreads.speech.SpeechVoice
import io.github.chflick.allreads.speech.pickVoice
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

class VoicePickTest {
    private val canadaOffline = SpeechVoice("fr-ca-local", "fr-CA", offline = true)
    private val franceNetwork = SpeechVoice("fr-fr-network", "fr-FR", offline = false)
    private val franceOffline = SpeechVoice("fr-fr-local", "fr-FR", offline = true)
    private val all = listOf(canadaOffline, franceNetwork, franceOffline)

    @Test
    fun theChosenVoiceWins() = assertEquals(canadaOffline, pickVoice(all, "fr-FR", "fr-ca-local"))

    @Test
    fun theExactLocaleOfflineComesFirst() = assertEquals(franceOffline, pickVoice(all, "fr-FR", null))

    @Test
    fun aChosenVoiceThatWasRemovedFallsBack() = assertEquals(franceOffline, pickVoice(all, "fr-FR", "gone"))

    @Test
    fun anotherRegionIsBetterThanNothing() =
        assertEquals(canadaOffline, pickVoice(listOf(canadaOffline), "fr-FR", null))

    @Test
    fun noVoiceForTheLanguage() =
        assertNull(pickVoice(listOf(SpeechVoice("de", "de-DE", true)), "fr-FR", null))
}
