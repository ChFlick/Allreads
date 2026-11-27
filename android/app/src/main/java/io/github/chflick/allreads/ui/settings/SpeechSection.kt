package io.github.chflick.allreads.ui.settings

import io.github.chflick.allreads.ui.AppText
import androidx.compose.ui.res.stringResource
import io.github.chflick.allreads.R
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Slider
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.chflick.allreads.data.AppSettings
import io.github.chflick.allreads.data.Services
import io.github.chflick.allreads.speech.SpeakResult
import io.github.chflick.allreads.speech.SpeechVoice
import io.github.chflick.allreads.speech.pickVoice
import io.github.chflick.allreads.ui.reader.openTtsSettings
import java.util.Locale
import kotlinx.coroutines.launch

/** A language in the library, with the voices the phone has for it. */
private data class LanguageVoices(val code: String, val name: String, val localeTag: String, val voices: List<SpeechVoice>)

/** Step 25: speed, speak-on-tap, and a voice per language in the library. */
@Composable
fun SpeechSection(services: Services) {
    val settings by services.settings.settings.collectAsStateWithLifecycle()
    var languages by remember { mutableStateOf<List<LanguageVoices>?>(null) }
    LaunchedEffect(Unit) {
        languages = runCatching {
            services.repository.library().map { it.languageCode }.distinct().sorted().map { code ->
                val info = services.repository.languageInfo(code)
                val tag = info?.ttsLocale?.ifBlank { null } ?: code
                LanguageVoices(code, info?.name ?: code, tag, services.speech.voices(tag))
            }
        }.getOrDefault(emptyList())
    }

    var rate by remember(settings.speechRate) { mutableFloatStateOf(settings.speechRate) }
    Text(stringResource(R.string.speed, "%.2f".format(rate)), style = MaterialTheme.typography.labelLarge)
    Slider(
        value = rate,
        onValueChange = { rate = it },
        onValueChangeFinished = { services.settings.update { it.copy(speechRate = rate) } },
        valueRange = AppSettings.speechRates,
        steps = 9,
        colors = calmSliderColors(),
    )
    SwitchSetting(
        title = AppText.get(R.string.speak_words_when_tapped),
        description = AppText.get(R.string.otherwise_use_the_play_button),
        checked = settings.speakOnTap,
        onCheckedChange = { on -> services.settings.update { it.copy(speakOnTap = on) } },
    )
    languages?.forEach { VoiceChoice(services, it, settings) }
}

@Composable
private fun VoiceChoice(services: Services, language: LanguageVoices, settings: AppSettings) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var problem by remember { mutableStateOf<String?>(null) }
    if (language.voices.isEmpty()) {
        Text(stringResource(R.string.no_voice_on_this_phone, language.name), style = MaterialTheme.typography.bodyMedium)
        OutlinedButton(onClick = { openTtsSettings(context) }) { Text(stringResource(R.string.get_a_voice, language.name)) }
        return
    }
    val chosen = settings.voices[language.code]
    val current = pickVoice(language.voices, language.localeTag, chosen)
    SettingsDropdown(
        label = "${language.name} voice",
        selected = current?.let { voiceLabel(it) } ?: AppText.get(R.string.default_label),
        options = language.voices.map { voiceLabel(it) to it.name },
        onSelect = { name -> services.settings.update { it.copy(voices = it.voices + (language.code to name)) } },
    )
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        OutlinedButton(onClick = {
            scope.launch {
                val result = services.speech.speak(
                    sampleSentence(language.localeTag), language.localeTag, chosen, settings.speechRate, "test",
                )
                problem = if (result == SpeakResult.Started) null else AppText.get(R.string.the_voice_could_not_speak)
            }
        }) { Text(stringResource(R.string.test)) }
        if (chosen != null) {
            OutlinedButton(onClick = {
                services.settings.update { it.copy(voices = it.voices - language.code) }
            }) { Text(stringResource(R.string.use_default)) }
        }
    }
    problem?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error) }
}

/** E.g. "French (Canada) · fr-ca-x-cad-local · offline". */
private fun voiceLabel(voice: SpeechVoice): String {
    val where = Locale.forLanguageTag(voice.localeTag).displayName
    return AppText.get(R.string.voice_label, where, voice.name, AppText.get(if (voice.offline) R.string.voice_offline else R.string.voice_needs_network))
}

private fun sampleSentence(localeTag: String): String = when (localeTag.substringBefore('-').lowercase()) {
    "fr" -> AppText.get(R.string.bonjour_voici_comment_je_lis)
    "de" -> AppText.get(R.string.hallo_so_lese_ich_deine)
    "es" -> AppText.get(R.string.hola_as_leo_tus_libros)
    "en" -> AppText.get(R.string.hello_this_is_how_i)
    else -> Locale.forLanguageTag(localeTag).let { it.getDisplayLanguage(it) }
}
