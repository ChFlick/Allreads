package io.github.chflick.allreads.ui.settings

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
    Text("Speed: ${"%.2f".format(rate)}×", style = MaterialTheme.typography.labelLarge)
    Slider(
        value = rate,
        onValueChange = { rate = it },
        onValueChangeFinished = { services.settings.update { it.copy(speechRate = rate) } },
        valueRange = AppSettings.speechRates,
        steps = 9,
        colors = calmSliderColors(),
    )
    Row(verticalAlignment = Alignment.CenterVertically) {
        Column(Modifier.weight(1f)) {
            Text("Speak words when tapped", style = MaterialTheme.typography.bodyMedium)
            Text("Otherwise use the play button in the word drawer.", style = MaterialTheme.typography.bodySmall)
        }
        Switch(
            checked = settings.speakOnTap,
            onCheckedChange = { on -> services.settings.update { it.copy(speakOnTap = on) } },
        )
    }
    languages?.forEach { VoiceChoice(services, it, settings) }
}

@Composable
private fun VoiceChoice(services: Services, language: LanguageVoices, settings: AppSettings) {
    val context = LocalContext.current
    val scope = rememberCoroutineScope()
    var problem by remember { mutableStateOf<String?>(null) }
    if (language.voices.isEmpty()) {
        Text("No ${language.name} voice on this phone.", style = MaterialTheme.typography.bodyMedium)
        OutlinedButton(onClick = { openTtsSettings(context) }) { Text("Get a ${language.name} voice") }
        return
    }
    val chosen = settings.voices[language.code]
    val current = pickVoice(language.voices, language.localeTag, chosen)
    SettingsDropdown(
        label = "${language.name} voice",
        selected = current?.let { voiceLabel(it) } ?: "Default",
        options = language.voices.map { voiceLabel(it) to it.name },
        onSelect = { name -> services.settings.update { it.copy(voices = it.voices + (language.code to name)) } },
    )
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        OutlinedButton(onClick = {
            scope.launch {
                val result = services.speech.speak(
                    sampleSentence(language.localeTag), language.localeTag, chosen, settings.speechRate, "test",
                )
                problem = if (result == SpeakResult.Started) null else "The voice could not speak."
            }
        }) { Text("Test") }
        if (chosen != null) {
            OutlinedButton(onClick = {
                services.settings.update { it.copy(voices = it.voices - language.code) }
            }) { Text("Use default") }
        }
    }
    problem?.let { Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error) }
}

/** E.g. "French (Canada) · fr-ca-x-cad-local · offline". */
private fun voiceLabel(voice: SpeechVoice): String {
    val where = Locale.forLanguageTag(voice.localeTag).displayName
    return "$where · ${voice.name} · ${if (voice.offline) "offline" else "needs network"}"
}

private fun sampleSentence(localeTag: String): String = when (localeTag.substringBefore('-').lowercase()) {
    "fr" -> "Bonjour ! Voici comment je lis vos livres."
    "de" -> "Hallo! So lese ich deine Bücher vor."
    "es" -> "¡Hola! Así leo tus libros en voz alta."
    "en" -> "Hello! This is how I read your books aloud."
    else -> Locale.forLanguageTag(localeTag).let { it.getDisplayLanguage(it) }
}
