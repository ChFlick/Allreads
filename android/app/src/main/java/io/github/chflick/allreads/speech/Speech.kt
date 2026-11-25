package io.github.chflick.allreads.speech

import kotlinx.coroutines.flow.StateFlow

/** An installed voice, e.g. French (France), offline. */
data class SpeechVoice(
    /** The engine's own name for it, stored in Settings. */
    val name: String,
    /** BCP 47 tag of its locale, e.g. `fr-FR`. */
    val localeTag: String,
    /** Works without network. */
    val offline: Boolean,
)

/** What happened when asked to speak. */
enum class SpeakResult {
    Started,

    /** No voice for the language is installed; offer the text-to-speech settings. */
    NoVoice,

    /** The phone's text-to-speech engine is missing or failed to start. */
    Unavailable,
}

/**
 * Read-aloud (step 25). An interface so a cloud voice can be added later as
 * another implementation; [AndroidSpeech] uses the phone's voices.
 */
interface Speech {
    /** The id of what is being spoken now, or `null` when quiet. */
    val speaking: StateFlow<String?>

    /** Installed voices for a language (any region), for the voice picker. */
    suspend fun voices(localeTag: String): List<SpeechVoice>

    /**
     * Speaks `text`, replacing anything being spoken. `voiceName` is the voice
     * chosen in Settings, or `null` for the best installed one; `rate` is 1.0
     * for normal speed. `id` comes back through [speaking].
     */
    suspend fun speak(text: String, localeTag: String, voiceName: String?, rate: Float, id: String): SpeakResult

    fun stop()
}
