package io.github.chflick.allreads.ui.settings

import androidx.compose.ui.res.stringResource
import androidx.annotation.StringRes
import io.github.chflick.allreads.R
import androidx.activity.compose.BackHandler
import androidx.compose.foundation.background
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.size
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.automirrored.filled.KeyboardArrowRight
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.produceState
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.ui.platform.LocalContext
import io.github.chflick.allreads.anki.AnkiAccess
import io.github.chflick.allreads.ui.theme.Literata
import androidx.compose.foundation.BorderStroke
import androidx.compose.material3.SliderDefaults
import androidx.compose.material3.Surface
import io.github.chflick.allreads.data.ReaderPage
import io.github.chflick.allreads.data.ReadingFont
import io.github.chflick.allreads.ui.languageName
import io.github.chflick.allreads.ui.theme.ReaderPageTheme
import io.github.chflick.allreads.ui.theme.family

import android.os.Build
import androidx.compose.material3.Switch

import androidx.compose.material3.AlertDialog
import androidx.compose.material3.TextButton
import androidx.compose.ui.platform.LocalUriHandler

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ExperimentalLayoutApi
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
import androidx.compose.runtime.mutableFloatStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.em
import androidx.compose.ui.unit.sp
import androidx.lifecycle.compose.collectAsStateWithLifecycle
import io.github.chflick.allreads.BuildConfig
import io.github.chflick.allreads.data.AppSettings
import io.github.chflick.allreads.data.Services
import io.github.chflick.allreads.data.ThemeMode
import io.github.chflick.allreads.ui.NO_DEEPL_KEY
import io.github.chflick.allreads.ui.describe
import uniffi.allreads_ffi.AllreadsException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/** The pages Settings opens from its overview. */
private enum class SettingsPage(@StringRes val title: Int) {
    Translation(R.string.translation),
    Anki(R.string.ankidroid),
    Dictionaries(R.string.dictionaries),
    Text(R.string.text_and_page),
    ReadAloud(R.string.read_aloud),
    Appearance(R.string.appearance),
    About(R.string.about_allreads),
}

/**
 * Settings: a short overview whose rows say what is set ("DeepL · into
 * English"), each opening its page. Problems show in the summary line, so
 * nobody has to open a page to find them.
 */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
fun SettingsScreen(services: Services) {
    var page by rememberSaveable { mutableStateOf<SettingsPage?>(null) }
    BackHandler(enabled = page != null) { page = null }
    val open = page
    if (open == null) {
        SettingsOverview(services, onOpen = { page = it })
        return
    }
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(open.title)) },
                navigationIcon = {
                    IconButton(onClick = { page = null }) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = stringResource(R.string.back_to_settings))
                    }
                },
            )
        },
    ) { padding ->
        Column(
            Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState()).padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(12.dp),
        ) {
            when (open) {
                SettingsPage.Translation -> TranslationSettings(services)
                SettingsPage.Anki -> AnkiSection(services)
                SettingsPage.Dictionaries -> DictionarySettings(services)
                SettingsPage.Text -> ReadingSettings(services)
                SettingsPage.ReadAloud -> SpeechSection(services)
                SettingsPage.Appearance -> AppearanceSettings(services)
                SettingsPage.About -> AboutSettings()
            }
        }
    }
}

@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsOverview(services: Services, onOpen: (SettingsPage) -> Unit) {
    val settings by services.settings.settings.collectAsStateWithLifecycle()
    val keyHint by services.apiKeys.deeplKeyHint.collectAsStateWithLifecycle()
    val dictionaries by services.dictionaries.list.collectAsStateWithLifecycle()
    val waitingOrNull by services.ankiSync.waiting.collectAsStateWithLifecycle()
    val waiting = waitingOrNull ?: 0
    val ankiAccess by produceState<AnkiAccess?>(null) { value = withContext(Dispatchers.IO) { services.anki.access() } }
    LaunchedEffect(Unit) { services.dictionaries.refresh() }

    Scaffold(topBar = { TopAppBar(title = { Text(stringResource(R.string.settings)) }) }) { padding ->
        Column(Modifier.fillMaxSize().padding(padding).verticalScroll(rememberScrollState())) {
            GroupLabel("Words and cards")
            OverviewRow("文A", "Translation", onClick = { onOpen(SettingsPage.Translation) },
                summary = if (keyHint == null) "No DeepL key yet" else "DeepL · into ${languageName(settings.nativeLanguage)}",
                problem = keyHint == null)
            OverviewRow("A", "AnkiDroid", onClick = { onOpen(SettingsPage.Anki) },
                summary = when (ankiAccess) {
                    null -> ""
                    AnkiAccess.NotInstalled -> "AnkiDroid is not installed"
                    AnkiAccess.NeedsPermission -> "Not connected yet"
                    AnkiAccess.Connected -> if (waiting > 0) "Connected · $waiting words waiting" else "Connected"
                },
                problem = ankiAccess == AnkiAccess.NeedsPermission || waiting > 0)
            OverviewRow("D", "Dictionaries", onClick = { onOpen(SettingsPage.Dictionaries) },
                summary = dictionaries.installed.takeIf { it.isNotEmpty() }
                    ?.joinToString { "${languageName(it.pair.substringBefore('-'))} → English" }
                    ?.let { "$it installed" } ?: "None installed")
            GroupLabel("Reading")
            OverviewRow("Aa", "Text and page", onClick = { onOpen(SettingsPage.Text) },
                summary = "${settings.readingFont.label} · ${(settings.fontScale * 100).toInt()} % · ${settings.readerPage.label}",
                serif = true)
            OverviewRow("▶", "Read aloud", onClick = { onOpen(SettingsPage.ReadAloud) },
                summary = "${"%.1f".format(settings.speechRate)}× · speak on tap ${if (settings.speakOnTap) "on" else "off"}")
            OverviewRow("◐", "Appearance", onClick = { onOpen(SettingsPage.Appearance) },
                summary = "${settings.theme.name} theme · ${if (settings.wallpaperColors) "wallpaper colours" else "app colours"}")
            GroupLabel("About")
            OverviewRow("i", "About Allreads", onClick = { onOpen(SettingsPage.About) },
                summary = "${BuildConfig.VERSION_NAME} · licences · source code")
        }
    }
}

@Composable
private fun GroupLabel(text: String) {
    Text(
        text.uppercase(),
        style = MaterialTheme.typography.labelSmall.copy(letterSpacing = 1.sp),
        color = MaterialTheme.colorScheme.onSurfaceVariant,
        modifier = Modifier.padding(start = 16.dp, top = 20.dp, bottom = 4.dp),
    )
}

/** One overview row: a small tile, the page's name, and a summary of what is set. */
@Composable
private fun OverviewRow(icon: String, title: String, summary: String, onClick: () -> Unit, problem: Boolean = false, serif: Boolean = false) {
    Row(
        Modifier.fillMaxWidth().clickable(onClick = onClick).padding(horizontal = 16.dp, vertical = 12.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(14.dp),
    ) {
        Box(
            Modifier.size(38.dp).background(MaterialTheme.colorScheme.primaryContainer, MaterialTheme.shapes.medium),
            contentAlignment = Alignment.Center,
        ) {
            Text(
                icon,
                style = MaterialTheme.typography.titleSmall,
                fontFamily = if (serif) Literata else null,
                color = MaterialTheme.colorScheme.onPrimaryContainer,
            )
        }
        Column(Modifier.weight(1f)) {
            Text(title, style = MaterialTheme.typography.bodyLarge)
            if (summary.isNotEmpty()) {
                Text(
                    summary,
                    style = MaterialTheme.typography.bodySmall,
                    color = if (problem) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
        }
        Icon(Icons.AutoMirrored.Filled.KeyboardArrowRight, contentDescription = null, tint = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}

@Composable
private fun TranslationSettings(services: Services) {
    val settings by services.settings.settings.collectAsStateWithLifecycle()
    val hasKey by services.apiKeys.hasDeeplKey.collectAsStateWithLifecycle()
    DeeplKeyField(services, hasKey)
    SettingsDropdown(
        label = "Translate into",
        selected = languageName(settings.nativeLanguage),
        options = AppSettings.nativeLanguages.map { languageName(it) to it },
        onSelect = { code -> services.settings.update { it.copy(nativeLanguage = code) } },
    )
}

@Composable
private fun DictionarySettings(services: Services) {
    val settings by services.settings.settings.collectAsStateWithLifecycle()
    DictionarySection(services.dictionaries)
    SwitchSetting(
        title = "Meanings in the book's language",
        description = "The word drawer shows synonyms and dictionary forms first. Translations stay hidden " +
            "until you tap Reveal; they still load in the background, so revealing is instant.",
        checked = settings.hideTranslations,
        onCheckedChange = { on -> services.settings.update { it.copy(hideTranslations = on) } },
    )
}

/**
 * Text size, spacing, font, page colours and the double tap, with a preview of
 * the page. Also the reader's "Text and page" sheet, so it can be changed while reading.
 */
@Composable
fun ReadingSettings(services: Services) {
    val settings by services.settings.settings.collectAsStateWithLifecycle()
    // Sliders move a local value while dragging and save once on release.
    var fontScale by remember(settings.fontScale) { mutableFloatStateOf(settings.fontScale) }
    var lineSpacing by remember(settings.lineSpacing) { mutableFloatStateOf(settings.lineSpacing) }
    Column(verticalArrangement = Arrangement.spacedBy(12.dp)) {
        // The preview shows the page as the reader will: its font, size, spacing and colours.
        ReaderPageTheme(settings.readerPage) {
            Surface(
                color = MaterialTheme.colorScheme.background,
                shape = MaterialTheme.shapes.medium,
                border = BorderStroke(1.dp, MaterialTheme.colorScheme.outlineVariant),
                modifier = Modifier.fillMaxWidth(),
            ) {
                Text(
                    stringResource(R.string.il_tait_une_fois_un),
                    fontFamily = settings.readingFont.family(),
                    fontSize = 19.sp * fontScale,
                    lineHeight = lineSpacing.em,
                    color = MaterialTheme.colorScheme.onSurface,
                    modifier = Modifier.padding(16.dp),
                )
            }
        }
        Text(stringResource(R.string.text_size, (fontScale * 100).toInt()), style = MaterialTheme.typography.labelLarge)
        Slider(
            value = fontScale,
            onValueChange = { fontScale = it },
            onValueChangeFinished = { services.settings.update { it.copy(fontScale = fontScale) } },
            valueRange = AppSettings.fontScales,
            colors = calmSliderColors(),
        )
        Text(stringResource(R.string.line_spacing, "%.1f".format(lineSpacing)), style = MaterialTheme.typography.labelLarge)
        Slider(
            value = lineSpacing,
            onValueChange = { lineSpacing = it },
            onValueChangeFinished = { services.settings.update { it.copy(lineSpacing = lineSpacing) } },
            valueRange = AppSettings.lineSpacings,
            colors = calmSliderColors(),
        )
        Text(stringResource(R.string.font), style = MaterialTheme.typography.labelLarge)
        ChoiceChips(
            choices = ReadingFont.entries,
            selected = settings.readingFont,
            label = { it.label },
            onSelect = { font -> services.settings.update { it.copy(readingFont = font) } },
            font = { it.family() },
        )
        Text(stringResource(R.string.page), style = MaterialTheme.typography.labelLarge)
        ChoiceChips(
            choices = ReaderPage.entries,
            selected = settings.readerPage,
            label = { it.label },
            onSelect = { page -> services.settings.update { it.copy(readerPage = page) } },
        )
        SwitchSetting(
            title = "Double-tap to raise a level",
            description = "A double tap on a word you are learning moves it up one level (5 becomes known) " +
                "without opening the drawer. Single taps then wait a moment for a possible second tap.",
            checked = settings.doubleTapRaises,
            onCheckedChange = { on -> services.settings.update { it.copy(doubleTapRaises = on) } },
        )
    }
}

@Composable
private fun AppearanceSettings(services: Services) {
    val settings by services.settings.settings.collectAsStateWithLifecycle()
    ChoiceChips(
        choices = ThemeMode.entries,
        selected = settings.theme,
        label = { it.name },
        onSelect = { mode -> services.settings.update { it.copy(theme = mode) } },
    )
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        SwitchSetting(
            title = "Use the wallpaper's colours",
            checked = settings.wallpaperColors,
            onCheckedChange = { on -> services.settings.update { it.copy(wallpaperColors = on) } },
        )
    }
}

/** What Allreads builds on, and under which licence. */
private val credits = listOf(
    "Literata" to "reading font · SIL Open Font License 1.1",
    "Wiktionary, via kaikki.org" to "dictionary data · CC BY-SA 4.0",
    "Lukaisu" to "language rules · MIT / Unlicense",
    "DeepL" to "translations, with your own key",
    "AnkiDroid API" to "the connection to AnkiDroid",
    "Android Jetpack and Compose" to "Apache License 2.0",
    "Rust libraries" to "rusqlite, scraper, rbook, zip, ruzstd, encoding_rs, ureq, uniffi · MIT / Apache 2.0",
)

@Composable
private fun AboutSettings() {
    val uri = LocalUriHandler.current
    val context = LocalContext.current
    var showOfl by remember { mutableStateOf(false) }
    Text(stringResource(R.string.allreads_mit_licence, BuildConfig.VERSION_NAME), style = MaterialTheme.typography.titleMedium)
    Text(
        "Privacy: translations go to DeepL with your own key, and cards go to AnkiDroid on this phone. " +
            "Nothing else leaves it.",
        style = MaterialTheme.typography.bodyMedium,
    )
    TextButton(onClick = { uri.openUri("https://github.com/ChFlick/Allreads") }, contentPadding = PaddingValues(0.dp)) {
        Text(stringResource(R.string.source_code_and_issues_on))
    }
    HorizontalDivider()
    Text(stringResource(R.string.built_with), style = MaterialTheme.typography.titleSmall)
    credits.forEach { (name, what) ->
        Column {
            Text(name, style = MaterialTheme.typography.bodyMedium)
            Text(what, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
    TextButton(onClick = { showOfl = true }, contentPadding = PaddingValues(0.dp)) { Text(stringResource(R.string.read_the_literata_licence)) }
    if (showOfl) {
        val text = remember {
            runCatching { context.assets.open("licences/Literata-OFL.txt").bufferedReader().use { it.readText() } }
                .getOrDefault("The licence text is missing from this build.")
        }
        AlertDialog(
            onDismissRequest = { showOfl = false },
            title = { Text(stringResource(R.string.sil_open_font_license)) },
            text = { Text(text, style = MaterialTheme.typography.bodySmall, modifier = Modifier.verticalScroll(rememberScrollState())) },
            confirmButton = { TextButton(onClick = { showOfl = false }) { Text(stringResource(R.string.close)) } },
        )
    }
}

/** Sliders with a quiet grey track, not the theme's amber. */
@Composable
internal fun calmSliderColors() = SliderDefaults.colors(
    inactiveTrackColor = MaterialTheme.colorScheme.surfaceContainerHighest,
    inactiveTickColor = MaterialTheme.colorScheme.onSurfaceVariant,
)

@Suppress("unused")
@Composable
private fun Section(title: String) {
    Text(title, style = MaterialTheme.typography.titleMedium, color = MaterialTheme.colorScheme.primary)
}

/**
 * A key-check failure as shown on this page. Unlike the reader's messages, it
 * never says "check Settings": the user is already here.
 */
private fun keyCheckMessage(error: Throwable): String = when (error) {
    is AllreadsException.TranslationKeyRejected -> "DeepL rejected this key."
    else -> describe(error)
}

/** The result of checking the saved DeepL key. */
private sealed interface KeyCheck {
    data object Checking : KeyCheck
    data class Works(val used: ULong?, val limit: ULong?) : KeyCheck
    data class Failed(val message: String) : KeyCheck
}

/**
 * Paste, replace, check or remove the DeepL key. A newly saved key is checked
 * at once against DeepL's usage endpoint, which costs no quota. The saved key
 * is never shown, only its last four characters.
 */
@Composable
private fun DeeplKeyField(services: Services, hasKey: Boolean) {
    var input by rememberSaveable { mutableStateOf("") }
    val hint by services.apiKeys.deeplKeyHint.collectAsStateWithLifecycle()
    var check by remember { mutableStateOf<KeyCheck?>(null) }
    val scope = rememberCoroutineScope()

    fun runCheck(newKey: String?) {
        check = KeyCheck.Checking
        scope.launch {
            // Keystore work and the network call both stay off the main thread.
            val key = withContext(Dispatchers.IO) {
                newKey?.also { services.apiKeys.saveDeeplKey(it) } ?: services.apiKeys.deeplKey()
            }
            check = if (key == null) {
                KeyCheck.Failed(NO_DEEPL_KEY)
            } else {
                runCatching { services.repository.checkDeeplKey(key) }.fold(
                    { KeyCheck.Works(it.charactersUsed, it.characterLimit) },
                    { KeyCheck.Failed(keyCheckMessage(it)) },
                )
            }
        }
    }

    val uri = LocalUriHandler.current
    var confirmRemove by remember { mutableStateOf(false) }
    Text(
        if (hint != null) "DeepL API key saved (…$hint)" else "No DeepL API key yet. DeepL's free plan translates 500,000 characters a month.",
        style = MaterialTheme.typography.bodyMedium,
    )
    if (hint == null) {
        TextButton(onClick = { uri.openUri("https://www.deepl.com/pro-api") }) { Text(stringResource(R.string.get_a_free_key_at)) }
    }
    if (confirmRemove) {
        AlertDialog(
            onDismissRequest = { confirmRemove = false },
            title = { Text(stringResource(R.string.remove_the_deepl_key)) },
            text = { Text(stringResource(R.string.translations_stop_working_until_you)) },
            confirmButton = {
                TextButton(onClick = {
                    confirmRemove = false
                    services.apiKeys.clearDeeplKey()
                    check = null
                }) { Text(stringResource(R.string.remove)) }
            },
            dismissButton = { TextButton(onClick = { confirmRemove = false }) { Text(stringResource(R.string.cancel)) } },
        )
    }
    when (val result = check) {
        null -> Unit
        KeyCheck.Checking -> Text(stringResource(R.string.checking_the_key_with_deepl), style = MaterialTheme.typography.bodySmall)
        is KeyCheck.Works -> Text(
            if (result.used != null && result.limit != null) {
                "✓ The key works · ${"%,d".format(result.used.toLong())} of " +
                    "${"%,d".format(result.limit.toLong())} characters used this month"
            } else {
                "✓ The key works"
            },
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.primary,
        )
        is KeyCheck.Failed -> Text(
            result.message,
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.error,
        )
    }
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
                val key = input.trim()
                input = ""
                runCheck(key)
            },
        ) { Text(stringResource(R.string.save_key)) }
        if (hasKey) {
            OutlinedButton(onClick = { runCheck(null) }) { Text(stringResource(R.string.check_key)) }
            OutlinedButton(onClick = { confirmRemove = true }) { Text(stringResource(R.string.remove)) }
        }
    }
}
