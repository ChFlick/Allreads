package io.github.chflick.allreads.speech

import android.content.Context
import android.speech.tts.TextToSpeech
import android.speech.tts.UtteranceProgressListener
import java.util.Locale
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update

/**
 * [Speech] with the phone's text-to-speech engine. The engine starts on first
 * use and stays for the life of the app process.
 */
class AndroidSpeech(context: Context) : Speech {
    private val context = context.applicationContext
    private val ready = CompletableDeferred<Boolean>()
    private val _speaking = MutableStateFlow<String?>(null)
    override val speaking: StateFlow<String?> = _speaking.asStateFlow()

    private val tts: TextToSpeech by lazy {
        TextToSpeech(this.context) { status -> ready.complete(status == TextToSpeech.SUCCESS) }.apply {
            setOnUtteranceProgressListener(object : UtteranceProgressListener() {
                override fun onStart(utteranceId: String) {
                    _speaking.value = utteranceId
                }

                override fun onDone(utteranceId: String) = finished(utteranceId)

                @Deprecated("Deprecated in Java")
                override fun onError(utteranceId: String) = finished(utteranceId)

                override fun onError(utteranceId: String, errorCode: Int) = finished(utteranceId)

                override fun onStop(utteranceId: String, interrupted: Boolean) = finished(utteranceId)
            })
        }
    }

    private fun finished(id: String) = _speaking.update { if (it == id) null else it }

    private suspend fun started(): Boolean {
        tts // starts the engine
        return ready.await()
    }

    override suspend fun voices(localeTag: String): List<SpeechVoice> {
        if (!started()) return emptyList()
        val language = Locale.forLanguageTag(localeTag).language
        return runCatching { tts.voices.orEmpty() }.getOrDefault(emptySet())
            .filter { it.locale.language == language }
            .filterNot { TextToSpeech.Engine.KEY_FEATURE_NOT_INSTALLED in it.features.orEmpty() }
            .map { SpeechVoice(it.name, it.locale.toLanguageTag(), offline = !it.isNetworkConnectionRequired) }
            .sortedWith(compareBy({ !it.localeTag.equals(localeTag, true) }, { !it.offline }, { it.name }))
    }

    override suspend fun speak(text: String, localeTag: String, voiceName: String?, rate: Float, id: String): SpeakResult {
        if (!started()) return SpeakResult.Unavailable
        val voice = pickVoice(voices(localeTag), localeTag, voiceName)
        if (voice != null) {
            tts.voices.orEmpty().firstOrNull { it.name == voice.name }?.let { tts.voice = it }
        } else {
            // Some engines list no voices but still speak the language.
            val result = tts.setLanguage(Locale.forLanguageTag(localeTag))
            if (result < TextToSpeech.LANG_AVAILABLE) return SpeakResult.NoVoice
        }
        tts.setSpeechRate(rate)
        val queued = tts.speak(text, TextToSpeech.QUEUE_FLUSH, null, id)
        return if (queued == TextToSpeech.SUCCESS) SpeakResult.Started else SpeakResult.Unavailable
    }

    override fun stop() {
        if (ready.isCompleted) tts.stop()
        _speaking.value = null
    }
}
