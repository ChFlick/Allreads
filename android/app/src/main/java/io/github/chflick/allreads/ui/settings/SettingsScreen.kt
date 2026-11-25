package io.github.chflick.allreads.ui.settings

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
import androidx.compose.foundation.layout.FlowRow
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.FilterChip
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Slider
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.chflick.allreads.data.AppSettings
import io.github.chflick.allreads.data.Services
import io.github.chflick.allreads.data.ThemeMode
import java.util.Locale

/** Step 15: translation, reading and appearance settings. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalLayoutApi::class)
@Composable
fun SettingsScreen(services: Services) {
    val settings by services.settings.settings.collectAsStateWithLifecycle()
    val hasKey by services.apiKeys.hasDeeplKey.collectAsStateWithLifecycle()

    Scaffold(topBar = { TopAppBar(title = { Text("Settings") }) }) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            Section("Translation")
            DeeplKeyField(services, hasKey)
            Text("Translate into", style = MaterialTheme.typography.labelLarge)
            FlowRow(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                AppSettings.nativeLanguages.forEach { code ->
                    FilterChip(
                        selected = settings.nativeLanguage == code,
                        onClick = { services.settings.update { it.copy(nativeLanguage = code) } },
                        label = { Text(Locale.forLanguageTag(code).displayLanguage) },
                    )
                }
            }

            HorizontalDivider()
            Section("Reading")
            Text("Text size: ${(settings.fontScale * 100).toInt()} %", style = MaterialTheme.typography.labelLarge)
            Slider(
                value = settings.fontScale,
                onValueChange = { v -> services.settings.update { it.copy(fontScale = v) } },
                valueRange = AppSettings.fontScales,
            )
            Text("Line spacing: ${"%.1f".format(settings.lineSpacing)}", style = MaterialTheme.typography.labelLarge)
            Slider(
                value = settings.lineSpacing,
                onValueChange = { v -> services.settings.update { it.copy(lineSpacing = v) } },
                valueRange = AppSettings.lineSpacings,
            )
            Text(
                "Il était une fois un renard qui errait dans la forêt la nuit.",
                fontFamily = FontFamily.Serif,
                fontSize = 19.sp * settings.fontScale,
                lineHeight = settings.lineSpacing.em,
            )

            HorizontalDivider()
            Section("Appearance")
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                ThemeMode.entries.forEach { mode ->
                    FilterChip(
                        selected = settings.theme == mode,
                        onClick = { services.settings.update { it.copy(theme = mode) } },
                        label = { Text(mode.name) },
                    )
                }
            }

            HorizontalDivider()
            Section("About")
            Text(
                "Allreads 0.1.0 · MIT licence. Language rules adapted from Lukaisu. " +
                    "Translations by DeepL with your own key; nothing else leaves the phone.",
                style = MaterialTheme.typography.bodySmall,
            )
        }
    }
}

@Composable
private fun Section(title: String) {
    Text(title, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.primary)
}

/** Paste, replace or remove the DeepL key. The saved key is never shown, only its last four characters. */
@Composable
private fun DeeplKeyField(services: Services, hasKey: Boolean) {
    var input by rememberSaveable { mutableStateOf("") }
    val hint = remember(hasKey) { if (hasKey) services.apiKeys.deeplKeyHint() else null }
    Text(
        if (hint != null) "DeepL API key saved (…$hint)" else "No DeepL API key yet. Get a free one at deepl.com/pro-api.",
        style = MaterialTheme.typography.bodyMedium,
    )
    OutlinedTextField(
        value = input,
        onValueChange = { input = it },
        label = { Text(if (hasKey) "Replace the DeepL API key" else "DeepL API key") },
        singleLine = true,
        visualTransformation = PasswordVisualTransformation(),
        modifier = Modifier.fillMaxWidth(),
    )
    Row(horizontalArrangement = Arrangement.spacedBy(8.dp), verticalAlignment = Alignment.CenterVertically) {
        Button(
            enabled = input.isNotBlank(),
            onClick = {
                services.apiKeys.saveDeeplKey(input)
                input = ""
            },
        ) { Text("Save key") }
        if (hasKey) {
            OutlinedButton(onClick = { services.apiKeys.clearDeeplKey() }) { Text("Remove key") }
        }
    }
}
