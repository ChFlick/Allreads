package io.github.chflick.allreads.speech

/**
 * The voice to speak a language with: the one chosen in Settings if it is
 * still installed; otherwise one for the exact locale (fr-FR) before one for
 * the language only (fr-CA), and offline voices before network ones.
 */
fun pickVoice(voices: List<SpeechVoice>, localeTag: String, chosen: String?): SpeechVoice? {
    chosen?.let { name -> voices.firstOrNull { it.name == name }?.let { return it } }
    val language = localeTag.substringBefore('-').lowercase()
    val exact = { v: SpeechVoice -> v.localeTag.equals(localeTag, ignoreCase = true) }
    val sameLanguage = { v: SpeechVoice -> v.localeTag.substringBefore('-').lowercase() == language }
    return voices.firstOrNull { exact(it) && it.offline }
        ?: voices.firstOrNull(exact)
        ?: voices.firstOrNull { sameLanguage(it) && it.offline }
        ?: voices.firstOrNull(sameLanguage)
}
