package io.github.chflick.allreads.ui.settings

import io.github.chflick.allreads.ui.sharedAxisX
import androidx.compose.animation.AnimatedContent
import io.github.chflick.allreads.ui.AppText
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
fun SettingsScreen(services: Services, onVocabCheck: (String) -> Unit = {}, refresh: Int = 0) {
    var page by rememberSaveable { mutableStateOf<SettingsPage?>(null) }
    BackHandler(enabled = page != null) { page = null }
    // A page slides in from the overview and back out to it.
    AnimatedContent(targetState = page, transitionSpec = { sharedAxisX(forward = targetState != null) }, label = "settings") { open ->
        if (open == null) {
            SettingsOverview(services, onOpen = { page = it })
        } else {
            SettingsPageScreen(services, open, onBack = { page = null }, onVocabCheck = onVocabCheck, refresh = refresh)
        }
    }
}

/** One settings page, with a back arrow to the overview. */
@OptIn(ExperimentalMaterial3Api::class)
@Composable
private fun SettingsPageScreen(
    services: Services,
    open: SettingsPage,
    onBack: () -> Unit,
    onVocabCheck: (String) -> Unit,
    refresh: Int,
) {
    Scaffold(
        topBar = {
            TopAppBar(
                title = { Text(stringResource(open.title)) },
                navigationIcon = {
                    IconButton(onClick = onBack) {
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
                SettingsPage.Dictionaries -> DictionarySettings(services, onVocabCheck, refresh)
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
            GroupLabel(AppText.get(R.string.words_and_cards))
            OverviewRow("文A", AppText.get(R.string.translation), onClick = { onOpen(SettingsPage.Translation) },
                summary = if (keyHint == null) AppText.get(R.string.no_deepl_key_yet) else AppText.get(R.string.deepl_into, languageName(settings.nativeLanguage)),
                problem = keyHint == null)
            OverviewRow("A", AppText.get(R.string.ankidroid), onClick = { onOpen(SettingsPage.Anki) },
                summary = when (ankiAccess) {
                    null -> ""
                    AnkiAccess.NotInstalled -> AppText.get(R.string.ankidroid_is_not_installed)
                    AnkiAccess.NeedsPermission -> AppText.get(R.string.not_connected_yet)
                    AnkiAccess.Connected -> if (waiting > 0) AppText.plural(R.plurals.connected_words_waiting, waiting, waiting) else AppText.get(R.string.connected)
                },
                problem = ankiAccess == AnkiAccess.NeedsPermission || waiting > 0)
            OverviewRow("D", AppText.get(R.string.dictionaries), onClick = { onOpen(SettingsPage.Dictionaries) },
                summary = dictionaries.installed.takeIf { it.isNotEmpty() }
                    ?.joinToString { AppText.get(R.string.english, languageName(it.pair.substringBefore('-'))) }
                    ?.let { "$it installed" } ?: AppText.get(R.string.none_installed))
            GroupLabel(AppText.get(R.string.reading))
            OverviewRow(AppText.get(R.string.aa), AppText.get(R.string.text_and_page), onClick = { onOpen(SettingsPage.Text) },
                summary = "${settings.readingFont.label} · ${(settings.fontScale * 100).toInt()} % · ${settings.readerPage.label}",
                serif = true)
            OverviewRow("▶", AppText.get(R.string.read_aloud), onClick = { onOpen(SettingsPage.ReadAloud) },
                summary = AppText.get(if (settings.speakOnTap) R.string.speech_summary_on else R.string.speech_summary_off, settings.speechRate))
            OverviewRow("◐", AppText.get(R.string.appearance), onClick = { onOpen(SettingsPage.Appearance) },
                summary = AppText.get(R.string.theme, settings.theme.label, AppText.get(if (settings.wallpaperColors) R.string.wallpaper_colours else R.string.app_colours)))
            GroupLabel(AppText.get(R.string.about))
            OverviewRow("i", AppText.get(R.string.about_allreads), onClick = { onOpen(SettingsPage.About) },
                summary = AppText.get(R.string.licences_source_code, BuildConfig.VERSION_NAME))
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
        label = AppText.get(R.string.translate_into),
        selected = languageName(settings.nativeLanguage),
        options = AppSettings.nativeLanguages.map { languageName(it) to it },
        onSelect = { code -> services.settings.update { it.copy(nativeLanguage = code) } },
    )
}

@Composable
private fun DictionarySettings(services: Services, onVocabCheck: (String) -> Unit, refresh: Int) {
    val settings by services.settings.settings.collectAsStateWithLifecycle()
    DictionarySection(services.dictionaries)
    SwitchSetting(
        title = AppText.get(R.string.meanings_in_the_book_s),
        description = AppText.get(R.string.the_word_drawer_shows_synonyms),
        checked = settings.hideTranslations,
        onCheckedChange = { on -> services.settings.update { it.copy(hideTranslations = on) } },
    )
    VocabCheckSection(services, refresh, onStart = onVocabCheck)
    CatalogSection(services.catalogs)
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
            title = AppText.get(R.string.double_tap_to_raise_a),
            description = AppText.get(R.string.a_double_tap_on_a),
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
        label = { it.label },
        onSelect = { mode -> services.settings.update { it.copy(theme = mode) } },
    )
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
        SwitchSetting(
            title = AppText.get(R.string.use_the_wallpaper_s_colours),
            checked = settings.wallpaperColors,
            onCheckedChange = { on -> services.settings.update { it.copy(wallpaperColors = on) } },
        )
    }
}

/** What Allreads builds on, and under which licence. */
/** The credits on About, read each time, so they follow the phone's language. */
private fun credits() = listOf(
    AppText.get(R.string.literata) to AppText.get(R.string.reading_font_sil_open_font),
    AppText.get(R.string.wiktionary_via_kaikki_org) to AppText.get(R.string.dictionary_data_cc_by_sa),
    AppText.get(R.string.lexique_credit) to AppText.get(R.string.french_word_frequencies_cc_by_sa),
    AppText.get(R.string.frequencywords_credit) to AppText.get(R.string.other_word_frequencies_cc_by_sa),
    AppText.get(R.string.lukaisu) to AppText.get(R.string.language_rules_mit_unlicense),
    AppText.get(R.string.deepl_2) to AppText.get(R.string.translations_with_your_own_key),
    AppText.get(R.string.ankidroid_api) to AppText.get(R.string.the_connection_to_ankidroid),
    AppText.get(R.string.android_jetpack_and_compose) to AppText.get(R.string.apache_license_2_0),
    AppText.get(R.string.rust_libraries) to AppText.get(R.string.rusqlite_scraper_rbook_zip_ruzstd),
)

@Composable
private fun AboutSettings() {
    val uri = LocalUriHandler.current
    val context = LocalContext.current
    var showOfl by remember { mutableStateOf(false) }
    Text(stringResource(R.string.allreads_mit_licence, BuildConfig.VERSION_NAME), style = MaterialTheme.typography.titleMedium)
    Text(
        AppText.get(R.string.privacy_translations_go_to_deepl),
        style = MaterialTheme.typography.bodyMedium,
    )
    TextButton(onClick = { uri.openUri("https://github.com/ChFlick/Allreads") }, contentPadding = PaddingValues(0.dp)) {
        Text(stringResource(R.string.source_code_and_issues_on))
    }
    HorizontalDivider()
    Text(stringResource(R.string.built_with), style = MaterialTheme.typography.titleSmall)
    credits().forEach { (name, what) ->
        Column {
            Text(name, style = MaterialTheme.typography.bodyMedium)
            Text(what, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
    TextButton(onClick = { showOfl = true }, contentPadding = PaddingValues(0.dp)) { Text(stringResource(R.string.read_the_literata_licence)) }
    if (showOfl) {
        val text = remember {
            runCatching { context.assets.open("licences/Literata-OFL.txt").bufferedReader().use { it.readText() } }
                .getOrDefault(AppText.get(R.string.the_licence_text_is_missing))
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
    is AllreadsException.TranslationKeyRejected -> AppText.get(R.string.deepl_rejected_this_key)
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
        hint?.let { AppText.get(R.string.deepl_api_key_saved, it) } ?: AppText.get(R.string.no_deepl_api_key_yet),
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
                AppText.get(R.string.the_key_works_usage, result.used.toLong(), result.limit.toLong())
            } else {
                AppText.get(R.string.the_key_works)
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
        label = { Text(if (hasKey) AppText.get(R.string.replace_the_deepl_api_key) else AppText.get(R.string.deepl_api_key)) },
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
